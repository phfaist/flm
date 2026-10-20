  // --- the toolbar --------------------------------------------------------------------
  //
  // A bar at the bottom right corner of the window, laid over the page and not part of
  // the document. It shows the state of the build — a build under way, or the count of
  // the last build's problems, colored by the worst severity — and holds one button per
  // tool; a tool that shows something shows it in a panel above the bar. A line above the
  // bar tells what the page could not do for the author, until it next does it (`tell`).
  // The bar folds to its state alone. Each tool is one entry of `TOOLS`:
  //
  // - *Problems*: the list of the last build's problems (`problems.js`, which also marks
  //   the elements a problem lies in).
  // - *Rebuild*: asks the server to build the document again now.
  // - *Outputs*: the command lines that write this document's standalone outputs.
  // - *Settings*: what the front matter writes of the document's features, changed and
  //   saved (`settings.js`).
  //
  // An entry gives its label and either what it shows (`show`, given the panel each time
  // the toolbar is shown anew) or what it does on a click (`act`); one that shows something
  // may also give what to do when its panel closes (`closed`).
  //
  // After the tools, a menu of the editors that the server found on this machine (the
  // `editors` of its message `hello`) chooses the editor that a place opens in
  // (`gestures.js` sends the choice with each request). The browser remembers the choice
  // for the server's address, across loadings of the page and runs of `flm watch` at the
  // same port; the menu shows it when the server found that editor, else the server's
  // default (the `editor` of `hello`). The menu is hidden when no editor was found.

  // Where the toolbar keeps, for this tab, whether it is folded and which tool is open,
  // so that a page loaded anew shows it as it was.
  var KEPT = 'flm-preview-toolbar';

  // Where the browser keeps the name of the editor chosen in the menu, for the server's
  // address. See decision rationale at [§flm-dd-dr:host-watch-editor-menu].
  var EDITOR_KEPT = 'flm-preview-editor';

  var TOOLS = [
    { name: 'problems', label: 'Problems', show: showProblems },
    { name: 'rebuild', label: 'Rebuild', act: requestRebuild },
    { name: 'outputs', label: 'Outputs', show: showOutputs },
    { name: 'settings', label: 'Settings', show: showSettings, closed: forgetSettings }
  ];

  // What the toolbar shows, and its elements once made.
  var view = {
    building: false,
    document: null,
    folded: false,
    open: null,
    root: null,
    state: null,
    panel: null,
    notice: null,
    told: null,
    buttons: {},
    // The name of the editor chosen in the menu of editors (`null` when the menu lists
    // none), and the menu.
    editor: null,
    menu: null
  };

  function keep() {
    try {
      window.sessionStorage.setItem(KEPT, JSON.stringify({ folded: view.folded, open: view.open }));
    } catch (error) {
      // A page that may not keep anything shows the toolbar unfolded next time.
    }
  }

  function kept() {
    try {
      var value = JSON.parse(window.sessionStorage.getItem(KEPT));
      return value !== null && typeof value === 'object' ? value : null;
    } catch (error) {
      return null;
    }
  }

  function keepEditor(name) {
    try {
      window.localStorage.setItem(EDITOR_KEPT, name);
    } catch (error) {
      // A page that may not keep anything shows the server's default next time.
    }
  }

  function keptEditor() {
    try {
      var name = window.localStorage.getItem(EDITOR_KEPT);
      return isText(name) ? name : null;
    } catch (error) {
      return null;
    }
  }

  // Lays the toolbar over the page; `open` is the tool open at first unless this tab
  // kept another choice.
  function makeToolbar(open) {
    var root = make('div', 'toolbar');
    root.setAttribute('role', 'region');
    root.setAttribute('aria-label', 'FLM preview');
    var panel = make('div', 'panel');
    var bar = make('div', 'bar');
    var state = make('button', 'state');
    state.type = 'button';
    state.addEventListener('click', function () {
      if (view.folded) {
        fold(false);
      } else {
        choose(view.open === 'problems' ? null : 'problems');
      }
    });
    bar.appendChild(state);
    var tools = make('span', 'tools');
    TOOLS.forEach(function (tool) {
      var button = make('button', 'tool', tool.label);
      button.type = 'button';
      button.setAttribute('data-flm-preview-tool', tool.name);
      button.addEventListener('click', function () {
        if (tool.act) {
          tool.act();
        } else {
          choose(view.open === tool.name ? null : tool.name);
        }
      });
      view.buttons[tool.name] = button;
      tools.appendChild(button);
    });
    bar.appendChild(tools);
    var menu = make('select', 'editor');
    menu.setAttribute('aria-label', 'Editor');
    menu.title = 'The editor in which a click with Ctrl or Cmd and Shift, or Open in editor, opens the source';
    // Shown once the server has listed the editors it found.
    menu.hidden = true;
    menu.addEventListener('change', function () {
      view.editor = menu.value;
      keepEditor(menu.value);
    });
    bar.appendChild(menu);
    var folding = make('button', 'fold', '–');
    folding.type = 'button';
    folding.title = 'Fold the toolbar';
    folding.setAttribute('aria-label', 'Fold the toolbar');
    folding.addEventListener('click', function () {
      fold(true);
    });
    bar.appendChild(folding);
    var notice = make('p', 'notice');
    notice.setAttribute('role', 'status');
    root.appendChild(panel);
    root.appendChild(notice);
    root.appendChild(bar);
    view.root = root;
    view.state = state;
    view.panel = panel;
    view.notice = notice;
    view.menu = menu;
    var before = kept();
    view.folded = before !== null && before.folded === true;
    view.open = before !== null ? toolNamed(before.open) : toolNamed(open);
    document.body.appendChild(root);
    render();
  }

  // `name` when it is the name of a tool that shows something, else `null`.
  function toolNamed(name) {
    for (var i = 0; i < TOOLS.length; i++) {
      if (TOOLS[i].name === name && TOOLS[i].show) {
        return name;
      }
    }
    return null;
  }

  // Tells the tool `name`, when it is open and gives what to do then, that its panel
  // closes.
  function closing(name) {
    TOOLS.forEach(function (tool) {
      if (tool.name === name && tool.closed) {
        tool.closed();
      }
    });
  }

  function fold(folded) {
    if (folded) {
      closing(view.open);
    }
    view.folded = folded;
    keep();
    render();
  }

  function choose(name) {
    if (toolNamed(name) !== view.open) {
      closing(view.open);
    }
    view.open = toolNamed(name);
    keep();
    render();
  }

  // Takes `given`, as the message `hello` carries it, as the editors the server found, and
  // `fallback` as the server's default; the menu then shows the editor this browser
  // remembers when it is among them, else the default.
  function setEditors(given, fallback) {
    var editors = Array.isArray(given) ? given.filter(function (editor) {
      return editor !== null && typeof editor === 'object' && isText(editor.name) && isText(editor.label);
    }) : [];
    var names = editors.map(function (editor) {
      return editor.name;
    });
    var remembered = keptEditor();
    var chosen = null;
    if (names.indexOf(remembered) !== -1) {
      chosen = remembered;
    } else if (names.indexOf(fallback) !== -1) {
      chosen = fallback;
    } else if (names.length > 0) {
      chosen = names[0];
    }
    view.editor = chosen;
    var menu = view.menu;
    if (menu === null) {
      return;
    }
    menu.textContent = '';
    editors.forEach(function (editor) {
      var option = make('option', null, editor.label);
      option.value = editor.name;
      menu.appendChild(option);
    });
    if (chosen !== null) {
      menu.value = chosen;
    }
    menu.hidden = editors.length === 0;
  }

  // Shows `text` above the bar, what the page could not do for the author; `null` takes
  // it away.
  function tell(text) {
    view.told = text;
    render();
  }

  function counted(count) {
    if (count === 0) {
      return 'No problems';
    }
    return count === 1 ? '1 problem' : count + ' problems';
  }

  // Shows what the toolbar holds now.
  function render() {
    if (view.root === null) {
      return;
    }
    view.root.toggleAttribute('data-flm-preview-folded', view.folded);
    var state = view.state;
    state.toggleAttribute('data-flm-preview-building', view.building);
    var severity = worst(lastProblems);
    if (severity === null) {
      state.removeAttribute('data-flm-preview-severity');
    } else {
      state.setAttribute('data-flm-preview-severity', severity);
    }
    state.textContent = view.building ? 'Building…' : counted(lastProblems.length);
    state.title = view.folded ? 'Unfold the toolbar' : 'Show or hide the problems of the last build';
    TOOLS.forEach(function (tool) {
      var button = view.buttons[tool.name];
      if (tool.show) {
        button.setAttribute('aria-expanded', String(view.open === tool.name));
      }
    });
    view.notice.textContent = view.told === null ? '' : view.told;
    view.notice.hidden = view.told === null;
    var panel = view.panel;
    panel.textContent = '';
    var tool = null;
    for (var i = 0; i < TOOLS.length; i++) {
      if (TOOLS[i].name === view.open) {
        tool = TOOLS[i];
      }
    }
    panel.hidden = view.folded || tool === null;
    if (!panel.hidden) {
      panel.setAttribute('data-flm-preview-tool', tool.name);
      tool.show(panel);
    }
  }

  // --- the tool Outputs ---

  // `word` as a shell reads it: as it is when it holds no character the shell treats
  // apart, else between single quotes.
  function shellWord(word) {
    return /^[A-Za-z0-9_.,:\/+=@%-]+$/.test(word) ? word : "'" + word.replace(/'/g, "'\\''") + "'";
  }

  // The command lines that write the standalone outputs of the document `name`, each
  // with what it does.
  function outputCommands(name) {
    var stem = name.replace(/\.flm$/, '');
    var input = shellWord(name);
    return [
      ['The complete HTML page:', 'flm render -p html -o ' + shellWord(stem + '.html') + ' ' + input],
      ['The LaTeX for LuaLaTeX, with everything resolved:', 'flm render -p latex-rendered -o ' + shellWord(stem + '.tex') + ' ' + input],
      ['The LaTeX source, written back close to the original:', 'flm render -p latex-recomposed -o ' + shellWord(stem + '.tex') + ' ' + input],
      ['The problems alone, without writing a file:', 'flm check ' + input]
    ];
  }

  function showOutputs(panel) {
    if (!isText(view.document)) {
      panel.appendChild(make('p', 'empty', 'Waiting for the server to send the document’s file name.'));
      return;
    }
    panel.appendChild(make('p', 'intro', 'The preview writes no file. To write this document’s outputs, run:'));
    var list = make('dl', 'outputs');
    outputCommands(view.document).forEach(function (entry) {
      list.appendChild(make('dt', null, entry[0]));
      var command = make('dd');
      command.appendChild(make('code', null, entry[1]));
      list.appendChild(command);
    });
    panel.appendChild(list);
  }

