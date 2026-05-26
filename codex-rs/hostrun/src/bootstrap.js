globalThis.ctx = globalThis.ctx ?? {};
globalThis.__hostrun_console = [];

globalThis.__hostrun_formatConsoleValue = function (value) {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch (_error) {
    return String(value);
  }
};

globalThis.__hostrun_consolePush = function (level, args) {
  globalThis.__hostrun_console.push({
    level,
    message: Array.from(args).map(globalThis.__hostrun_formatConsoleValue).join(" ")
  });
};

globalThis.console = {
  log: function (...args) { globalThis.__hostrun_consolePush("log", args); },
  info: function (...args) { globalThis.__hostrun_consolePush("info", args); },
  warn: function (...args) { globalThis.__hostrun_consolePush("warn", args); },
  error: function (...args) { globalThis.__hostrun_consolePush("error", args); },
  debug: function (...args) { globalThis.__hostrun_consolePush("debug", args); }
};

if (!Array.prototype.containing) {
  Object.defineProperty(Array.prototype, "containing", {
    value: function (needle) {
      return this.filter((value) => String(value).includes(String(needle)));
    },
    configurable: true,
    writable: true
  });
}

globalThis.__hostrun_defineArrayHelper = function (name, value) {
  if (!Array.prototype[name]) {
    Object.defineProperty(Array.prototype, name, {
      value,
      configurable: true,
      writable: true
    });
  }
};

globalThis.__hostrun_defineStringHelper = function (name, value) {
  if (!String.prototype[name]) {
    Object.defineProperty(String.prototype, name, {
      value,
      configurable: true,
      writable: true
    });
  }
};

globalThis.__hostrun_utf8ByteLength = function (value) {
  let bytes = 0;
  for (const char of String(value)) {
    const codePoint = char.codePointAt(0);
    if (codePoint <= 0x7f) {
      bytes += 1;
    } else if (codePoint <= 0x7ff) {
      bytes += 2;
    } else if (codePoint <= 0xffff) {
      bytes += 3;
    } else {
      bytes += 4;
    }
  }
  return bytes;
};

globalThis.__hostrun_defineStringHelper("lines", function () {
  const text = String(this);
  if (text.length === 0) {
    return [];
  }
  return text.replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n");
});

globalThis.__hostrun_defineStringHelper("json", function () {
  return JSON.parse(String(this));
});

globalThis.__hostrun_defineStringHelper("jsonLines", function () {
  return String(this).lines()
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line));
});

globalThis.__hostrun_defineStringHelper("lower", function () {
  return String(this).toLowerCase();
});

globalThis.__hostrun_defineStringHelper("upper", function () {
  return String(this).toUpperCase();
});

globalThis.__hostrun_defineStringHelper("bytes", function () {
  return globalThis.__hostrun_utf8ByteLength(this);
});

globalThis.__hostrun_defineStringHelper("chars", function () {
  return Array.from(String(this));
});

globalThis.__hostrun_regex = function (pattern) {
  return pattern instanceof RegExp ? pattern : new RegExp(String(pattern));
};

globalThis.__hostrun_formatField = function (value, transform, args) {
  let text = value === undefined ? "" : String(value);
  switch (transform) {
    case "":
      return text;
    case "trim":
      return text.trim();
    case "lower":
      return text.toLowerCase();
    case "upper":
      return text.toUpperCase();
    case "substr": {
      const parts = String(args).split(",").map((part) => Number(part.trim()));
      return text.substring(parts[0] || 0, parts.length > 1 ? parts[1] : undefined);
    }
    case "replace": {
      const [from, to = ""] = String(args).split(",");
      return text.replaceAll(from, to);
    }
    case "basename":
      return text.split("/").filter((part) => part.length > 0).pop() ?? "";
    case "dirname": {
      const parts = text.split("/");
      parts.pop();
      return parts.join("/") || ".";
    }
    default:
      throw new Error("unknown Hostrun field transform: " + transform);
  }
};

globalThis.__hostrun_formatTemplate = function (template, row) {
  if (typeof template === "string") {
    return String(template).replace(/\{(\d+)(?:\|([a-zA-Z]+)(?::([^}]*))?)?\}/g, function (_match, field, transform, args) {
      const index = Number(field) - 1;
      return globalThis.__hostrun_formatField(row[index], transform ?? "", args ?? "");
    });
  }
  const output = {};
  for (const [key, value] of Object.entries(template)) {
    output[key] = globalThis.__hostrun_formatTemplate(value, row);
  }
  return output;
};

globalThis.__hostrun_fieldTable = function (rows) {
  return {
    rows: function () {
      return rows;
    },
    format: function (template) {
      return rows.map((row) => globalThis.__hostrun_formatTemplate(template, row));
    },
    field: function (number) {
      const index = Number(number) - 1;
      return rows.map((row) => row[index] ?? "");
    }
  };
};

globalThis.__hostrun_defineArrayHelper("fields", function (separator = /\s+/) {
  return globalThis.__hostrun_fieldTable(
    this.map((line) => String(line).trim().split(separator).filter((field) => field.length > 0))
  );
});

globalThis.__hostrun_defineArrayHelper("notContaining", function (needle) {
  return this.filter((value) => !String(value).includes(String(needle)));
});

globalThis.__hostrun_defineArrayHelper("startsWith", function (prefix) {
  return this.filter((value) => String(value).startsWith(String(prefix)));
});

globalThis.__hostrun_defineArrayHelper("endsWith", function (suffix) {
  return this.filter((value) => String(value).endsWith(String(suffix)));
});

globalThis.__hostrun_defineArrayHelper("matching", function (pattern) {
  const regex = globalThis.__hostrun_regex(pattern);
  return this.filter((value) => regex.test(String(value)));
});

globalThis.__hostrun_defineArrayHelper("notMatching", function (pattern) {
  const regex = globalThis.__hostrun_regex(pattern);
  return this.filter((value) => !regex.test(String(value)));
});

globalThis.__hostrun_defineArrayHelper("first", function () {
  return this[0] ?? null;
});

globalThis.__hostrun_defineArrayHelper("last", function () {
  return this.length === 0 ? null : this[this.length - 1];
});

globalThis.__hostrun_defineArrayHelper("take", function (count) {
  return this.slice(0, Number(count));
});

globalThis.__hostrun_defineArrayHelper("unique", function () {
  return Array.from(new Set(this));
});

globalThis.__hostrun_defineArrayHelper("lengths", function () {
  return this.map((value) => String(value).length);
});

globalThis.__hostrun_defineArrayHelper("bytes", function () {
  return this.map((value) => String(value).bytes());
});

globalThis.__hostrun_defineArrayHelper("lower", function () {
  return this.map((value) => String(value).toLowerCase());
});

globalThis.__hostrun_defineArrayHelper("upper", function () {
  return this.map((value) => String(value).toUpperCase());
});

globalThis.__hostrun_defineArrayHelper("sorted", function () {
  return Array.from(this).sort();
});

globalThis.__hostrun_defineArrayHelper("reversed", function () {
  return Array.from(this).reverse();
});

globalThis.__hostrun_invokeCapability = function (path, payload) {
  const response = JSON.parse(globalThis.__hostrun_invokeTool(path, JSON.stringify(payload ?? {})));
  if (response.type === "needs_approval") {
    throw new Error("__HOSTRUN_APPROVAL_REQUIRED__:" + JSON.stringify(response.approval));
  }
  if (response.type === "denied") {
    throw new Error(response.reason);
  }
  return response.value;
};

globalThis.__hostrun_toolProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_toolProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      const payload = args.length > 0 ? args[0] : {};
      return globalThis.__hostrun_invokeCapability(path, payload);
    }
  });
};

globalThis.tools = globalThis.__hostrun_toolProxy("");

globalThis.fs = {
  write: function (path, content) {
    return globalThis.__hostrun_invokeCapability("fs.write", { path, content });
  },
  read: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.read", { path });
  },
  exists: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.exists", { path });
  },
  remove: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.remove", { path });
  }
};

globalThis.rclone = {
  deletefile: function (target) {
    return globalThis.__hostrun_invokeCapability("rclone.deletefile", { target });
  },
  lsf: function (target, options = {}) {
    const args = ["lsf", target];
    if (options.recursive) {
      args.push("--recursive");
    }
    return globalThis.__hostrun_commandBuilder("rclone", args);
  }
};

globalThis.__hostrun_commandBuilder = function (program, args) {
  const state = {
    program,
    args: Array.from(args)
  };
  const builder = {
    program: state.program,
    args: state.args,
    run: function () {
      return globalThis.__hostrun_invokeCapability("cli." + state.program, state);
    },
    toJSON: function () {
      return { ...state };
    }
  };
  const streamHandle = function (name) {
    return {
      stream: name,
      command: state,
      capture: function () {
        state[name] = { type: "capture" };
        return builder;
      },
      text: function () {
        state[name] = { type: "text" };
        return builder;
      },
      lines: function () {
        state[name] = { type: "lines" };
        return builder;
      },
      toFile: function (path) {
        state[name] = { type: "file", path };
        return builder;
      },
      toJSON: function () {
        return { stream: name, command: { program: state.program, args: state.args } };
      }
    };
  };
  builder.stdout = streamHandle("stdout");
  builder.stderr = streamHandle("stderr");
  builder.stderr.toStdout = function () {
    state.stderr = { type: "stdout" };
    return builder;
  };
  builder.combined = {
    capture: function () {
      state.combined = { type: "capture" };
      return builder;
    },
    toFile: function (path) {
      state.combined = { type: "file", path };
      return builder;
    }
  };
  const stdin = function (source) {
    state.stdin = { type: "stream", source };
    return builder;
  };
  stdin.text = function (text) {
    state.stdin = { type: "text", text };
    return builder;
  };
  stdin.file = function (path) {
    state.stdin = { type: "file", path };
    return builder;
  };
  stdin.json = function (value) {
    state.stdin = { type: "json", value };
    return builder;
  };
  stdin.lines = function (lines) {
    state.stdin = { type: "lines", lines };
    return builder;
  };
  builder.stdin = stdin;
  return builder;
};

globalThis.__hostrun_cliProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_cliProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      return globalThis.__hostrun_commandBuilder(path, args);
    }
  });
};

globalThis.cli = globalThis.__hostrun_cliProxy("");

globalThis.__hostrun_addOption = function (args, flag, value) {
  if (value === undefined || value === null || value === false) {
    return;
  }
  args.push(flag);
  if (value !== true) {
    args.push(String(value));
  }
};

globalThis.fd = {
  find: function (pattern, options = {}) {
    const args = [pattern];
    globalThis.__hostrun_addOption(args, "--type", options.type);
    globalThis.__hostrun_addOption(args, "--extension", options.extension);
    globalThis.__hostrun_addOption(args, "--max-depth", options.maxDepth);
    globalThis.__hostrun_addOption(args, "--absolute-path", options.absolutePath);
    globalThis.__hostrun_addOption(args, "--glob", options.glob);
    globalThis.__hostrun_addOption(args, "--hidden", options.hidden);
    globalThis.__hostrun_addOption(args, "--no-ignore", options.ignored === false);
    if (options.exclude) {
      for (const exclude of [].concat(options.exclude)) {
        args.push("--exclude", String(exclude));
      }
    }
    if (options.root) {
      args.push(String(options.root));
    }
    return globalThis.__hostrun_commandBuilder("fdfind", args);
  },
  files: function (root = ".", options = {}) {
    return globalThis.fd.find(".", { ...options, root, type: "file" });
  },
  dirs: function (root = ".", options = {}) {
    return globalThis.fd.find(".", { ...options, root, type: "directory" });
  }
};

globalThis.rg = {
  search: function (pattern, paths = [], options = {}) {
    const args = [];
    globalThis.__hostrun_addOption(args, "--fixed-strings", options.fixed);
    globalThis.__hostrun_addOption(args, "--ignore-case", options.ignoreCase);
    globalThis.__hostrun_addOption(args, "--json", options.json);
    globalThis.__hostrun_addOption(args, "--hidden", options.hidden);
    globalThis.__hostrun_addOption(args, "--no-ignore", options.ignored === false);
    globalThis.__hostrun_addOption(args, "--files-with-matches", options.filesWithMatches);
    globalThis.__hostrun_addOption(args, "--max-count", options.maxCount);
    globalThis.__hostrun_addOption(args, "--type", options.type);
    if (options.glob) {
      for (const glob of [].concat(options.glob)) {
        args.push("--glob", String(glob));
      }
    }
    if (options.context !== undefined) {
      args.push("--context", String(options.context));
    }
    args.push(String(pattern));
    args.push(...[].concat(paths).filter((path) => path !== undefined && path !== null).map(String));
    return globalThis.__hostrun_commandBuilder("rg", args);
  },
  files: function (pattern, paths = [], options = {}) {
    return globalThis.rg.search(pattern, paths, { ...options, filesWithMatches: true });
  },
  matches: function (pattern, paths = [], options = {}) {
    return globalThis.rg.search(pattern, paths, { ...options, json: true });
  }
};

globalThis.__hostrun_run = function (code) {
  globalThis.__hostrun_console = [];
  try {
    const value = (0, eval)(code);
    return JSON.stringify({
      type: "completed",
      executed: code,
      console: globalThis.__hostrun_console,
      value: value === undefined ? null : value
    });
  } catch (error) {
    const message = error && error.message ? String(error.message) : String(error);
    if (message.startsWith("__HOSTRUN_APPROVAL_REQUIRED__:")) {
      return message;
    }
    throw error;
  }
};
