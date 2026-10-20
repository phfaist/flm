//! A second extension compiled into the test program of `flm-ext/tests/native.rs`
//! beside the stub, written against the guest client `flm_ext::guest`, for the
//! scenarios that need two extensions or a failing handler.
//!
//! Its feature `second` declares the table `second.count`, written by the stage
//! `second.count` (one run per tree), which pushes the number of characters of the
//! tree; the macro `\secondmark{text}` (the element `second.mark`), whose rule
//! writes the lowered text between `[second:` and `]`; and the macro
//! `\secondpanic{text}` (the element `second.panic`), whose rule panics when its
//! text is `now` and writes the text otherwise; and the macro `\secondthread{text}`
//! (the element `second.thread`), whose rule sends a request (the media type of
//! `pdf`) from a thread it starts, records the answer, and writes the text. Its
//! `define` counts its runs on
//! each thread, one per instance created there, and sends a request (the media
//! type of `pdf`), which the host refuses since no handler call of this
//! extension runs; it records the answer on each thread. The rule's body of
//! `\secondpanic` sends the same request when it is dropped (with the instance
//! that recorded it), and records the answer likewise. Each record also says
//! whether the stub's rule was lowering its content on the thread
//! (`stub::lowering_content`): whether this code ran inside the stub's request.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use flm_ext::ExtError;
use flm_ext::guest::core::artifact::media_type_of;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::model::element::ElementSpec;
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

/// The table the stage writes.
pub const COUNT: &str = "second.count";

/// The text of the panic of `\secondpanic{now}`.
pub const PANIC: &str = "the second extension's rule was told to panic";

flm_ext::guest::extension!(extension);

/// A request's answer as this extension records it: whether the stub's rule was lowering
/// its content on the thread, and the error's text or the media type found.
pub type Answer = (bool, String);

thread_local! {
    /// How many times `define` ran on this thread.
    static DEFINES: Cell<usize> = const { Cell::new(0) };
    /// The answer to the request of each `define` run on this thread.
    static DEFINE_ANSWERS: RefCell<Vec<Answer>> = const { RefCell::new(Vec::new()) };
    /// The answer to the request of each drop of the rule's body of `\secondpanic` on
    /// this thread.
    static DROP_ANSWERS: RefCell<Vec<Answer>> = const { RefCell::new(Vec::new()) };
    /// The key under which the drops on this thread also record their answers in
    /// [`KEYED_DROP_ANSWERS`]; 0 for none. A `Cell` of a number has no destructor, so it
    /// can be read while the thread's thread-locals are destroyed.
    static DROP_KEY: Cell<u64> = const { Cell::new(0) };
}

/// The answers the drops on the threads that set a key ([`record_drops_under`]) recorded,
/// with the key: they can be read after such a thread ends.
static KEYED_DROP_ANSWERS: Mutex<Vec<(u64, Answer)>> = Mutex::new(Vec::new());

/// The answers to the requests the rule of `\secondthread` sent from the threads it
/// started, with the rule's text.
static THREAD_ANSWERS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The answer to the request the rule of `\secondthread{text}` sent from a thread it
/// started, for each `text` given (in the order the rules ran).
pub fn thread_answers_for(text: &str) -> Vec<String> {
    let answers = THREAD_ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    answers.iter().filter(|(given, _)| given == text).map(|(_, answer)| answer.clone()).collect()
}

/// How many times `define` ran on this thread: one per instance created there.
pub fn defines_on_this_thread() -> usize {
    DEFINES.with(Cell::get)
}

/// The answer to the request of each `define` run on this thread.
pub fn define_answers_on_this_thread() -> Vec<Answer> {
    DEFINE_ANSWERS.with(|answers| answers.borrow().clone())
}

/// The answer to the request of each drop of the rule's body of `\secondpanic` on this
/// thread: one per instance of this extension dropped there.
pub fn drop_answers_on_this_thread() -> Vec<Answer> {
    DROP_ANSWERS.with(|answers| answers.borrow().clone())
}

/// Makes the drops of the rule's body of `\secondpanic` on this thread also record their
/// answers under `key`, where they can be read after the thread ends
/// ([`drop_answers_under`]).
pub fn record_drops_under(key: u64) {
    DROP_KEY.with(|recorded| recorded.set(key));
}

/// The answers the drops recorded under `key` ([`record_drops_under`]), in their order.
pub fn drop_answers_under(key: u64) -> Vec<Answer> {
    let answers = KEYED_DROP_ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    answers.iter().filter(|(recorded, _)| *recorded == key).map(|(_, answer)| answer.clone()).collect()
}

/// Sends a request (the media type of `pdf`) and answers its answer.
fn send_a_request() -> Answer {
    let answer = match media_type_of("pdf") {
        Ok(found) => format!("found {found:?}"),
        Err(error) => error.to_string(),
    };
    (super::stub::lowering_content(), answer)
}

fn extension(ext: &mut Extension) {
    DEFINES.with(|count| count.set(count.get() + 1));
    let answer = send_a_request();
    DEFINE_ANSWERS.with(|answers| answers.borrow_mut().push(answer));
    ext.package("_", Package::new().uses(["stdcore"]).feature("second", Feature::new().title("Second").register(register)));
}

/// The characters of each tree.
struct Count;

impl Stage for Count {
    fn decl(&self) -> StageDecl {
        StageDecl::new("second.count", Effect::Compute, Scope::PerTree).writes(COUNT)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let tree = cx.tree_id()?.unwrap_or_default();
        let mut count = 0;
        if let Some(root) = cx.my_tree()? {
            for node in root.descendants()? {
                if node.is_chars()? {
                    count += node.chars()?.unwrap_or_default().chars().count();
                }
            }
        }
        out.table(COUNT)?.push(json!({ "tree": tree, "characters": count }))
    }
}

/// The rule's body of `\secondmark`.
struct Mark;

impl Lower for Mark {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let text = node.argument_content_nodes_named("text")?.unwrap_or_default();
        let mut staged = vec![lw.text(cx, "[second:")?];
        staged.extend(lw.lower_nodes(cx, &text)?);
        staged.push(lw.text(cx, "]")?);
        Ok(Lowering::Emit(staged))
    }
}

/// The rule's body of `\secondpanic`.
struct Panics;

impl Lower for Panics {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let text = chars_text(&node.argument_content_nodes_named("text")?.unwrap_or_default())?.unwrap_or_default();
        if text == "now" {
            panic!("{PANIC}");
        }
        Ok(Lowering::Emit(vec![lw.text(cx, &text)?]))
    }
}

/// The rule's body of `\secondthread`.
struct Thread;

impl Lower for Thread {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let text = chars_text(&node.argument_content_nodes_named("text")?.unwrap_or_default())?.unwrap_or_default();
        let answer = std::thread::spawn(|| match media_type_of("pdf") {
            Ok(found) => format!("found {found:?}"),
            Err(error) => error.to_string(),
        })
        .join()
        .unwrap_or_else(|_| "the thread panicked".to_string());
        THREAD_ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push((text.clone(), answer));
        Ok(Lowering::Emit(vec![lw.text(cx, &text)?]))
    }
}

impl Drop for Panics {
    /// Sends a request and records its answer: under the thread's key, if any, and on the
    /// thread, unless the thread's record is already destroyed (at the thread's end).
    fn drop(&mut self) {
        let answer = send_a_request();
        let key = DROP_KEY.with(Cell::get);
        if key != 0 {
            let mut keyed = KEYED_DROP_ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            keyed.push((key, answer.clone()));
        }
        let _ = DROP_ANSWERS.try_with(|answers| answers.borrow_mut().push(answer));
    }
}

/// Declares the macro `name` of one argument `text`, the element `element`, lowered by
/// `body` under the rule `ELEMENT.lower`.
fn declare(b: &mut FeatureBuilder, owner: &str, name: &str, element: &str, body: Arc<dyn Lower>) -> Result<(), ExtError> {
    let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] };
    b.element(ElementDef::new(owner, [name], syntax, ElementSpec::new(element)))?;
    b.lowering_rule(LoweringRule::new(format!("{element}.lower"), element, LoweringDecl::new(), body, DefaultMark::Everywhere)?)
}

fn register(b: &mut FeatureBuilder, _settings: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    b.table(COUNT, "the characters of each tree")?;
    b.stage(Arc::new(Count))?;
    declare(b, &owner, "secondmark", "second.mark", Arc::new(Mark))?;
    declare(b, &owner, "secondpanic", "second.panic", Arc::new(Panics))?;
    declare(b, &owner, "secondthread", "second.thread", Arc::new(Thread))
}
