//! Scripted guests of the `wasm` binding: a WebAssembly module, written in
//! the text format, that answers each entry call by a script instead of
//! running an extension's code. The one helper that builds every test guest.
//!
//! A script is a list of steps. Each entry call takes the first step not yet
//! used whose entry name is the call's (a step marked [`Step::repeat`] is
//! never used up); the step sends its requests through `flm.call` in order
//! (burning fuel where it says so), then answers its reply, or fails: a trap
//! (`unreachable`), a loop (the fuel runs out), memory grown until refused
//! then a trap, a panic message kept for `flm_last_panic` then a trap. An
//! entry call with no step left answers an `err` reply saying so.
//!
//! In a request's text and a reply's text, `$H1`, `$H2`, … stand for the
//! first, second, … `{"@handle": …}` of the entry's request (`null` when it
//! has fewer); `$R` for the `ok` value of the previous request's reply (the
//! whole reply when it is an `err`), and `$R0`, `$R1`, … for the first,
//! second, … item of that value, a list.
//!
//! The guest sends every entry request, and every reply it receives, to the
//! host's `console` (level `log`) as `entry TEXT` and `reply TEXT`, so that a
//! test reads what the guest received from its console sink. The used steps
//! are marked in the instance's memory: each instance runs the script from
//! its start.

use std::fmt::Write as _;

/// How a step ends.
#[derive(Debug, Clone)]
enum End {
    /// Answers this reply's text (with the substitutions).
    Reply(String),
    /// `unreachable`.
    Trap,
    /// A loop that never ends: the fuel runs out.
    Spin,
    /// `memory.grow` of 16 pages until refused, then `unreachable`.
    Grow,
    /// The message kept for `flm_last_panic`, then `unreachable`.
    Panic(String),
}

/// What a step does before it ends.
#[derive(Debug, Clone)]
enum Action {
    /// Sends this request's text (with the substitutions).
    Request(String),
    /// Runs a loop of this many iterations.
    Burn(u32),
}

/// One step of a script: the entry it answers, what it does, how it ends.
#[derive(Debug, Clone)]
pub struct Step {
    entry: String,
    actions: Vec<Action>,
    end: End,
    repeat: bool,
}

impl Step {
    /// A step answering the entry `entry` with `{"ok":null}`.
    pub fn new(entry: &str) -> Step {
        Step { entry: entry.to_string(), actions: Vec::new(), end: End::Reply(r#"{"ok":null}"#.to_string()), repeat: false }
    }

    /// Sends the request `text`.
    pub fn request(mut self, text: &str) -> Step {
        self.actions.push(Action::Request(text.to_string()));
        self
    }

    /// Runs a loop of `iterations` iterations.
    pub fn burn(mut self, iterations: u32) -> Step {
        self.actions.push(Action::Burn(iterations));
        self
    }

    /// Answers `{"ok": value}`, `value` a JSON text.
    pub fn ok(mut self, value: &str) -> Step {
        self.end = End::Reply(format!(r#"{{"ok":{value}}}"#));
        self
    }

    /// Answers the reply text `text` as it is.
    pub fn reply(mut self, text: &str) -> Step {
        self.end = End::Reply(text.to_string());
        self
    }

    /// Traps (`unreachable`).
    pub fn trap(mut self) -> Step {
        self.end = End::Trap;
        self
    }

    /// Loops until the fuel runs out.
    pub fn spin(mut self) -> Step {
        self.end = End::Spin;
        self
    }

    /// Grows its memory until refused, then traps.
    pub fn grow(mut self) -> Step {
        self.end = End::Grow;
        self
    }

    /// Keeps `message` as its panic message, then traps.
    pub fn panic(mut self, message: &str) -> Step {
        self.end = End::Panic(message.to_string());
        self
    }

    /// Never used up: answers every call of its entry.
    pub fn repeat(mut self) -> Step {
        self.repeat = true;
        self
    }
}

/// A script: its steps, and the initial memory in pages (computed when
/// absent).
#[derive(Debug, Clone)]
pub struct Script {
    steps: Vec<Step>,
    pages: Option<u32>,
}

/// The static data of a module: bytes placed from an offset, each string
/// at its own place.
struct Data {
    base: u32,
    bytes: Vec<u8>,
}

impl Data {
    /// Places `bytes`; answers their offset and length.
    fn add(&mut self, bytes: &[u8]) -> (u32, u32) {
        let at = self.base + self.bytes.len() as u32;
        self.bytes.extend_from_slice(bytes);
        (at, bytes.len() as u32)
    }
}

/// The fixed part of every scripted guest: the imports, the allocator, the
/// output buffer, the substitutions, the requests, the console log.
/// `@HEAP@`, `@PAGES@` and the offsets of the fixed texts are filled in.
const RUNTIME: &str = r#"
  (import "flm" "call" (func $call (param i32 i32) (result i32)))
  (import "flm" "take" (func $take (param i32 i32)))
  (memory (export "memory") @PAGES@)
  (global $heap (mut i32) (i32.const @HEAP@))
  (global $start (mut i32) (i32.const 0))
  (global $out (mut i32) (i32.const 0))
  (global $req (mut i32) (i32.const 0))
  (global $req_len (mut i32) (i32.const 0))
  (global $prev (mut i32) (i32.const 0))
  (global $prev_len (mut i32) (i32.const 0))
  (global $found (mut i32) (i32.const 0))
  (global $found_len (mut i32) (i32.const 0))
  (global $panic (mut i64) (i64.const 0))

  (func $ensure (param $end i32)
    (block $done
      (loop $more
        (br_if $done (i32.le_u (local.get $end) (i32.mul (memory.size) (i32.const 65536))))
        (if (i32.eq (memory.grow (i32.const 1)) (i32.const -1)) (then unreachable))
        (br $more))))

  (func $alloc (export "flm_alloc") (param $len i32) (result i32)
    (local $at i32)
    (local.set $at (global.get $heap))
    (global.set $heap (i32.and (i32.add (i32.add (local.get $at) (local.get $len)) (i32.const 7)) (i32.const -8)))
    (call $ensure (global.get $heap))
    (local.get $at))

  (func (export "flm_free") (param i32 i32))

  (func (export "flm_last_panic") (result i64) (global.get $panic))

  (func $emit (param $b i32)
    (call $ensure (i32.add (global.get $out) (i32.const 1)))
    (i32.store8 (global.get $out) (local.get $b))
    (global.set $out (i32.add (global.get $out) (i32.const 1))))

  (func $emit_range (param $p i32) (param $n i32)
    (block $done (loop $more
      (br_if $done (i32.eqz (local.get $n)))
      (call $emit (i32.load8_u (local.get $p)))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $more))))

  (func $emit_escaped (param $p i32) (param $n i32)
    (local $b i32)
    (block $done (loop $more
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $b (i32.load8_u (local.get $p)))
      (if (i32.or (i32.eq (local.get $b) (i32.const 34)) (i32.eq (local.get $b) (i32.const 92)))
        (then (call $emit (i32.const 92))))
      (call $emit (local.get $b))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $more))))

  (func $begin (global.set $start (global.get $heap)) (global.set $out (global.get $heap)))

  (func $end (global.set $heap (i32.and (i32.add (global.get $out) (i32.const 7)) (i32.const -8))))

  (func $eq (param $p i32) (param $q i32) (param $n i32) (result i32)
    (block $no (loop $more
      (if (i32.eqz (local.get $n)) (then (return (i32.const 1))))
      (br_if $no (i32.ne (i32.load8_u (local.get $p)) (i32.load8_u (local.get $q))))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (local.set $q (i32.add (local.get $q) (i32.const 1)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $more)))
    (i32.const 0))

  (func $is_entry (param $name i32) (param $len i32) (result i32)
    (if (i32.lt_u (global.get $req_len) (i32.add (local.get $len) (i32.const 11))) (then (return (i32.const 0))))
    (if (i32.eqz (call $eq (global.get $req) (i32.const @ENTRY_HEAD@) (i32.const 10))) (then (return (i32.const 0))))
    (if (i32.eqz (call $eq (i32.add (global.get $req) (i32.const 10)) (local.get $name) (local.get $len)))
      (then (return (i32.const 0))))
    (i32.eq (i32.load8_u (i32.add (i32.add (global.get $req) (i32.const 10)) (local.get $len))) (i32.const 34)))

  (func $find_handle (param $d i32) (result i32)
    (local $i i32) (local $end i32)
    (local.set $i (global.get $req))
    (local.set $end (i32.add (global.get $req) (global.get $req_len)))
    (block $none (loop $more
      (br_if $none (i32.gt_u (i32.add (local.get $i) (i32.const 11)) (local.get $end)))
      (if (call $eq (local.get $i) (i32.const @HANDLE_HEAD@) (i32.const 11))
        (then
          (local.set $d (i32.sub (local.get $d) (i32.const 1)))
          (if (i32.eqz (local.get $d))
            (then
              (global.set $found (local.get $i))
              (block $closed (loop $scan
                (br_if $none (i32.ge_u (i32.add (local.get $i) (i32.const 1)) (local.get $end)))
                (br_if $closed (i32.and (i32.eq (i32.load8_u (local.get $i)) (i32.const 125))
                  (i32.eq (i32.load8_u (i32.add (local.get $i) (i32.const 1))) (i32.const 125))))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $scan)))
              (global.set $found_len (i32.sub (i32.add (local.get $i) (i32.const 2)) (global.get $found)))
              (return (i32.const 1))))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $more)))
    (i32.const 0))

  ;; The `ok` value of the previous reply, into $found and $found_len.
  (func $ok_value
    (if (i32.and (i32.ge_u (global.get $prev_len) (i32.const 7)) (call $eq (global.get $prev) (i32.const @OK_HEAD@) (i32.const 6)))
      (then
        (global.set $found (i32.add (global.get $prev) (i32.const 6)))
        (global.set $found_len (i32.sub (global.get $prev_len) (i32.const 7))))
      (else
        (global.set $found (global.get $prev))
        (global.set $found_len (global.get $prev_len)))))

  ;; The item $i of the list [$p, $p + $n), into $found and $found_len.
  (func $item (param $p i32) (param $n i32) (param $i i32) (result i32)
    (local $end i32) (local $depth i32) (local $in i32) (local $esc i32) (local $start i32) (local $b i32)
    (local.set $end (i32.add (local.get $p) (local.get $n)))
    (local.set $p (i32.add (local.get $p) (i32.const 1)))
    (local.set $start (local.get $p))
    (block $out (loop $scan
      (br_if $out (i32.ge_u (local.get $p) (local.get $end)))
      (local.set $b (i32.load8_u (local.get $p)))
      (if (local.get $in)
        (then
          (if (local.get $esc)
            (then (local.set $esc (i32.const 0)))
            (else
              (if (i32.eq (local.get $b) (i32.const 92))
                (then (local.set $esc (i32.const 1)))
                (else (if (i32.eq (local.get $b) (i32.const 34)) (then (local.set $in (i32.const 0)))))))))
        (else
          (if (i32.eq (local.get $b) (i32.const 34)) (then (local.set $in (i32.const 1))))
          (if (i32.or (i32.eq (local.get $b) (i32.const 91)) (i32.eq (local.get $b) (i32.const 123)))
            (then (local.set $depth (i32.add (local.get $depth) (i32.const 1)))))
          (if (i32.or (i32.eq (local.get $b) (i32.const 93)) (i32.eq (local.get $b) (i32.const 125)))
            (then
              (if (i32.eqz (local.get $depth))
                (then
                  (if (i32.eqz (local.get $i))
                    (then
                      (global.set $found (local.get $start))
                      (global.set $found_len (i32.sub (local.get $p) (local.get $start)))
                      (return (i32.const 1))))
                  (return (i32.const 0))))
              (local.set $depth (i32.sub (local.get $depth) (i32.const 1)))))
          (if (i32.and (i32.eq (local.get $b) (i32.const 44)) (i32.eqz (local.get $depth)))
            (then
              (if (i32.eqz (local.get $i))
                (then
                  (global.set $found (local.get $start))
                  (global.set $found_len (i32.sub (local.get $p) (local.get $start)))
                  (return (i32.const 1))))
              (local.set $i (i32.sub (local.get $i) (i32.const 1)))
              (local.set $start (i32.add (local.get $p) (i32.const 1)))))))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (br $scan)))
    (i32.const 0))

  ;; Emits the template [$p, $p + $n) with $R, $R<d> and $H<d> substituted.
  (func $expand (param $p i32) (param $n i32)
    (local $b i32) (local $c i32) (local $d i32)
    (block $done (loop $more
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $b (i32.load8_u (local.get $p)))
      (local.set $c (i32.const 0))
      (local.set $d (i32.const 0))
      (if (i32.gt_u (local.get $n) (i32.const 1)) (then (local.set $c (i32.load8_u (i32.add (local.get $p) (i32.const 1))))))
      (if (i32.gt_u (local.get $n) (i32.const 2)) (then (local.set $d (i32.load8_u (i32.add (local.get $p) (i32.const 2))))))
      (if (i32.and (i32.eq (local.get $b) (i32.const 36)) (i32.eq (local.get $c) (i32.const 72)))
        (then
          (if (call $find_handle (i32.sub (local.get $d) (i32.const 48)))
            (then (call $emit_range (global.get $found) (global.get $found_len)))
            (else (call $emit_range (i32.const @NULL@) (i32.const 4))))
          (local.set $p (i32.add (local.get $p) (i32.const 3)))
          (local.set $n (i32.sub (local.get $n) (i32.const 3)))
          (br $more)))
      (if (i32.and (i32.eq (local.get $b) (i32.const 36)) (i32.eq (local.get $c) (i32.const 82)))
        (then
          (call $ok_value)
          (if (i32.and (i32.ge_u (local.get $d) (i32.const 48)) (i32.le_u (local.get $d) (i32.const 57)))
            (then
              (if (call $item (global.get $found) (global.get $found_len) (i32.sub (local.get $d) (i32.const 48)))
                (then (call $emit_range (global.get $found) (global.get $found_len)))
                (else (call $emit_range (i32.const @NULL@) (i32.const 4))))
              (local.set $p (i32.add (local.get $p) (i32.const 3)))
              (local.set $n (i32.sub (local.get $n) (i32.const 3)))
              (br $more)))
          (call $emit_range (global.get $found) (global.get $found_len))
          (local.set $p (i32.add (local.get $p) (i32.const 2)))
          (local.set $n (i32.sub (local.get $n) (i32.const 2)))
          (br $more)))
      (call $emit (local.get $b))
      (local.set $p (i32.add (local.get $p) (i32.const 1)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $more))))

  ;; Sends [$start, $out) as a request; keeps its reply as the previous one.
  (func $send
    (local $n i32) (local $at i32)
    (call $end)
    (local.set $n (call $call (global.get $start) (i32.sub (global.get $out) (global.get $start))))
    (local.set $at (call $alloc (local.get $n)))
    (call $take (local.get $at) (local.get $n))
    (global.set $prev (local.get $at))
    (global.set $prev_len (local.get $n)))

  ;; Logs `TAG TEXT` through `console`; the previous reply and the heap kept.
  (func $log (param $tag i32) (param $tag_len i32) (param $p i32) (param $n i32)
    (local $prev i32) (local $prev_len i32) (local $heap i32)
    (local.set $prev (global.get $prev))
    (local.set $prev_len (global.get $prev_len))
    (local.set $heap (global.get $heap))
    (call $begin)
    (call $emit_range (i32.const @CONSOLE_HEAD@) (i32.const @CONSOLE_HEAD_LEN@))
    (call $emit_range (local.get $tag) (local.get $tag_len))
    (call $emit_escaped (local.get $p) (local.get $n))
    (call $emit (i32.const 34))
    (call $emit (i32.const 125))
    (call $send)
    (global.set $prev (local.get $prev))
    (global.set $prev_len (local.get $prev_len))
    (global.set $heap (local.get $heap)))

  (func $request (param $p i32) (param $n i32)
    (call $begin)
    (call $expand (local.get $p) (local.get $n))
    (call $send)
    (call $log (i32.const @REPLY_TAG@) (i32.const 6) (global.get $prev) (global.get $prev_len)))

  (func $reply (param $p i32) (param $n i32) (result i64)
    (call $begin)
    (call $expand (local.get $p) (local.get $n))
    (call $end)
    (i64.or
      (i64.shl (i64.extend_i32_u (global.get $start)) (i64.const 32))
      (i64.extend_i32_u (i32.sub (global.get $out) (global.get $start)))))

  (func $burn (param $n i32)
    (block $done (loop $more
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $more))))
"#;

impl Script {
    /// A script of `steps`.
    pub fn new(steps: Vec<Step>) -> Script {
        Script { steps, pages: None }
    }

    /// The initial memory, in pages of 64 KiB.
    pub fn pages(mut self, pages: u32) -> Script {
        self.pages = Some(pages);
        self
    }

    /// The module in the text format.
    pub fn wat(&self) -> String {
        let flags = 16u32;
        let mut data = Data { base: (flags + self.steps.len() as u32 + 7) & !7, bytes: Vec::new() };
        let entry_head = data.add(br#"{"entry":""#);
        let handle_head = data.add(br#"{"@handle":"#);
        let ok_head = data.add(br#"{"ok":"#);
        let null = data.add(b"null");
        let console_head = data.add(br#"{"native":"console","level":"log","text":""#);
        let reply_tag = data.add(b"reply ");
        let entry_tag = data.add(b"entry ");
        let no_step = data.add(
            br#"{"err":{"kind":"handler","detail":"the scripted guest has no step left for this entry","data":null},"message":"the scripted guest has no step left for this entry"}"#,
        );
        let mut dispatch = String::new();
        for (index, step) in self.steps.iter().enumerate() {
            let flag = flags + index as u32;
            let name = data.add(step.entry.as_bytes());
            let unused = if step.repeat {
                "(i32.const 1)".to_string()
            } else {
                format!("(i32.eqz (i32.load8_u (i32.const {flag})))")
            };
            let _ = writeln!(dispatch, "    (if (i32.and {unused} (call $is_entry (i32.const {}) (i32.const {})))", name.0, name.1);
            dispatch.push_str("      (then\n");
            if !step.repeat {
                let _ = writeln!(dispatch, "        (i32.store8 (i32.const {flag}) (i32.const 1))");
            }
            for action in &step.actions {
                match action {
                    Action::Request(text) => {
                        let (at, len) = data.add(text.as_bytes());
                        let _ = writeln!(dispatch, "        (call $request (i32.const {at}) (i32.const {len}))");
                    }
                    Action::Burn(iterations) => {
                        let _ = writeln!(dispatch, "        (call $burn (i32.const {iterations}))");
                    }
                }
            }
            match &step.end {
                End::Reply(text) => {
                    let (at, len) = data.add(text.as_bytes());
                    let _ = writeln!(dispatch, "        (return (call $reply (i32.const {at}) (i32.const {len})))))");
                }
                End::Trap => dispatch.push_str("        unreachable))\n"),
                End::Spin => dispatch.push_str("        (loop $forever (br $forever))\n        unreachable))\n"),
                End::Grow => dispatch.push_str(
                    "        (block $refused (loop $again\n          (br_if $refused (i32.eq (memory.grow (i32.const 16)) (i32.const -1)))\n          (br $again)))\n        unreachable))\n",
                ),
                End::Panic(message) => {
                    let (at, len) = data.add(message.as_bytes());
                    let packed = (u64::from(at) << 32) | u64::from(len);
                    let _ = writeln!(dispatch, "        (global.set $panic (i64.const {packed}))\n        unreachable))");
                }
            }
        }
        let heap = (data.base + data.bytes.len() as u32 + 7) & !7;
        let pages = self.pages.unwrap_or(heap / 65536 + 2);
        let runtime = RUNTIME
            .replace("@PAGES@", &pages.to_string())
            .replace("@HEAP@", &heap.to_string())
            .replace("@ENTRY_HEAD@", &entry_head.0.to_string())
            .replace("@HANDLE_HEAD@", &handle_head.0.to_string())
            .replace("@OK_HEAD@", &ok_head.0.to_string())
            .replace("@NULL@", &null.0.to_string())
            .replace("@CONSOLE_HEAD@", &console_head.0.to_string())
            .replace("@CONSOLE_HEAD_LEN@", &console_head.1.to_string())
            .replace("@REPLY_TAG@", &reply_tag.0.to_string());
        let mut escaped = String::new();
        for byte in &data.bytes {
            let _ = write!(escaped, "\\{byte:02x}");
        }
        format!(
            "(module\n{runtime}\n  (data (i32.const {base}) \"{escaped}\")\n\n  (func (export \"flm_entry\") (param $ptr i32) (param $len i32) (result i64)\n    (global.set $req (local.get $ptr))\n    (global.set $req_len (local.get $len))\n    (call $log (i32.const {entry_tag}) (i32.const 6) (local.get $ptr) (local.get $len))\n{dispatch}    (call $reply (i32.const {no_step}) (i32.const {no_step_len}))))\n",
            base = data.base,
            entry_tag = entry_tag.0,
            no_step = no_step.0,
            no_step_len = no_step.1,
        )
    }

    /// The module's bytes.
    pub fn bytes(&self) -> Vec<u8> {
        wat_bytes(&self.wat())
    }
}

/// The bytes of the module `text`, in the text format.
pub fn wat_bytes(text: &str) -> Vec<u8> {
    wat::parse_str(text).unwrap_or_else(|error| panic!("the scripted guest is valid text: {error}\n{text}"))
}
