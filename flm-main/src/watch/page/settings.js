
  // --- the tool Settings ---------------------------------------------------------------
  //
  // The document's features, as the schema of a front matter lists them, and what the
  // front matter writes of each under `flm.features`: a section per feature, in the
  // schema's order, folded, with the feature's name, its description and a switch (on, or
  // off: `false` in the front matter). Unfolded, a section holds a form made from the
  // feature's schema by `@json-editor/json-editor`, with the values the front matter
  // writes: a key it does not write shows no value; the author adds a key to set it, or
  // removes it so that it is not written. *Save* sends the whole map of features to the
  // server, which writes the document's file anew; the build that follows is the one any
  // save of the file causes. See decision rationale at [§flm-dd-dr:host-watch-settings].

  var JSONEditor = module.exports.JSONEditor;

  var SETTINGS = '/.flm-preview/settings';

  // The open tool's view: its element, and one entry per feature of the schema. `null`
  // while the tool is closed: opening it reads the document's settings anew.
  var settingsView = null;

  // The options of the form library: a key not written is not shown and not in the value
  // until the author adds it; the plain theme, styled by the preview's stylesheet; no
  // editing as raw JSON, no reordering by dragging, no folding (the section folds), no
  // question before a removal.
  var FORM_OPTIONS = {
    theme: 'html',
    display_required_only: true,
    remove_empty_properties: false,
    disable_edit_json: true,
    disable_array_reorder: true,
    disable_collapse: true,
    prompt_before_delete: false,
    show_errors: 'interaction'
  };

  function showSettings(panel) {
    if (settingsView === null) {
      settingsView = { root: make('div', 'settings'), status: make('p', 'settings-status'), features: [], written: {} };
      settingsView.status.setAttribute('role', 'status');
      loadSettings(settingsView, null);
    }
    panel.appendChild(settingsView.root);
  }

  function forgetSettings() {
    settingsView = null;
  }

  // Shows `text` on the tool's line of status; `null` empties it.
  function settingsStatus(view, text) {
    view.status.textContent = text === null ? '' : text;
    view.status.hidden = text === null;
  }

  // Reads the document's settings from the server into `view`, then shows `told` on its
  // line of status.
  function loadSettings(view, told) {
    view.root.textContent = '';
    view.root.appendChild(make('p', 'empty', 'Reading the document’s settings…'));
    window.fetch(SETTINGS).then(function (answer) {
      if (!answer.ok) {
        return answer.text().then(function (text) {
          throw new Error(text !== '' ? text : 'The settings could not be read (the server answered with status ' + answer.status + ').');
        });
      }
      return answer.json();
    }).then(function (settings) {
      if (view === settingsView) {
        makeSettings(view, settings);
        settingsStatus(view, told);
      }
    }).catch(function (error) {
      if (view === settingsView) {
        view.root.textContent = '';
        view.root.appendChild(view.status);
        settingsStatus(view, error instanceof TypeError ? 'The preview’s server cannot be reached.' : error.message);
      }
    });
  }

  // The features the schema of a front matter `schema` lists, in its order: each one's
  // name, the schema of its settings with the schema's `$defs` beside it, and its
  // description.
  function featuresOf(schema) {
    var defs = schema.$defs || {};
    var listed = ((((schema.properties || {}).flm || {}).properties || {}).features || {}).properties || {};
    return Object.keys(listed).map(function (name) {
      var settings = (listed[name].anyOf || []).filter(function (branch) {
        return !('const' in branch) && branch.type !== 'null';
      })[0] || { type: 'object' };
      var reference = typeof settings.$ref === 'string' ? defs[settings.$ref.replace('#/$defs/', '')] : null;
      var description = (reference || settings).description;
      var form = Object.assign({}, settings, { $defs: defs });
      return { name: name, schema: form, description: typeof description === 'string' ? description : '' };
    });
  }

  // Fills `view` from `settings`, the server's response `{schema, features}`.
  function makeSettings(view, settings) {
    var written = settings.features !== null && typeof settings.features === 'object' && !Array.isArray(settings.features)
      ? settings.features : {};
    view.written = written;
    view.root.textContent = '';
    var head = make('div', 'settings-head');
    head.appendChild(make('p', 'intro',
      'The settings that the document’s front matter gives its features under flm.features. A setting the ' +
      'front matter does not give shows no value: the document then takes it from its packages or from the ' +
      'feature’s default. Save the file in your editor before saving here: saving here rewrites the file as it ' +
      'is on the disk, replacing only the values you changed.'));
    var save = make('button', 'settings-save', 'Save');
    save.type = 'button';
    save.addEventListener('click', function () {
      saveSettings(view);
    });
    head.appendChild(save);
    head.appendChild(view.status);
    view.root.appendChild(head);
    var list = make('div', 'settings-features');
    view.features = featuresOf(settings.schema || {}).map(function (feature) {
      feature.value = written[feature.name];
      feature.editor = null;
      list.appendChild(makeFeature(feature));
      return feature;
    });
    view.root.appendChild(list);
  }

  // The section of `feature`: its heading, with its switch, and its form, made when the
  // section is first unfolded.
  function makeFeature(feature) {
    var section = make('details', 'setting');
    section.setAttribute('data-flm-preview-feature', feature.name);
    var heading = make('summary', 'setting-heading');
    var toggle = make('input', 'setting-switch');
    toggle.type = 'checkbox';
    toggle.checked = feature.value !== false;
    toggle.title = 'Switch the feature on or off (off is saved as false in the front matter)';
    toggle.setAttribute('aria-label', 'Switch for the feature ' + feature.name);
    feature.toggle = toggle;
    heading.appendChild(toggle);
    heading.appendChild(make('code', 'setting-name', feature.name));
    section.appendChild(heading);
    section.appendChild(make('p', 'setting-description', feature.description));
    var form = make('div', 'setting-form');
    section.appendChild(form);
    section.addEventListener('toggle', function () {
      if (section.open && feature.editor === null) {
        var value = feature.value !== null && typeof feature.value === 'object' && !Array.isArray(feature.value)
          ? feature.value : {};
        feature.editor = new JSONEditor(form, Object.assign({ schema: feature.schema, startval: value }, FORM_OPTIONS));
      }
    });
    return section;
  }

  // What the front matter is to write of `feature`: `false` when it is off; the form's
  // value when its form was made, nothing when that is empty; else what the front matter
  // wrote, nothing when that is empty or `null`. `undefined` means that nothing is
  // written.
  function featureValue(feature) {
    if (!feature.toggle.checked) {
      return false;
    }
    var value = feature.editor !== null ? feature.editor.getValue() : feature.value;
    if (value === null || value === undefined || value === false) {
      return undefined;
    }
    if (typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === 0) {
      return undefined;
    }
    return value;
  }

  // The map of features to save: in the order the front matter wrote them, then the
  // others in the schema's order; a feature the schema does not list kept as written.
  function featuresToSave(view) {
    var byName = {};
    view.features.forEach(function (feature) {
      byName[feature.name] = feature;
    });
    var saved = {};
    var put = function (name, value) {
      if (value !== undefined) {
        saved[name] = value;
      }
    };
    Object.keys(view.written).forEach(function (name) {
      put(name, byName[name] ? featureValue(byName[name]) : view.written[name]);
    });
    view.features.forEach(function (feature) {
      if (!(feature.name in view.written)) {
        put(feature.name, featureValue(feature));
      }
    });
    return saved;
  }

  // Sends the features to the server, which writes the document's file; once written, the
  // tool reads the settings anew, which the file now holds.
  function saveSettings(view) {
    settingsStatus(view, 'Saving…');
    window.fetch(SETTINGS, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(featuresToSave(view))
    }).then(function (answer) {
      if (answer.ok) {
        if (view === settingsView) {
          loadSettings(view, 'Saved: the document is being built again.');
        }
        return null;
      }
      return answer.text().then(function (text) {
        settingsStatus(view, text !== '' ? text : 'The settings could not be saved (the server answered with status ' + answer.status + ').');
      });
    }).catch(function () {
      settingsStatus(view, 'The preview’s server cannot be reached.');
    });
  }
