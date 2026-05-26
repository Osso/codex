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

globalThis.__hostrun_definePrototypeHelper = function (prototype, name, value) {
  if (Object.prototype.hasOwnProperty.call(prototype, name)) {
    return;
  }
  const descriptor = Object.create(null);
  descriptor.value = value;
  descriptor.configurable = true;
  descriptor.writable = true;
  Object.defineProperty(prototype, name, descriptor);
};

globalThis.__hostrun_defineArrayHelper = function (name, value) {
  globalThis.__hostrun_definePrototypeHelper(Array.prototype, name, value);
};

globalThis.__hostrun_defineStringHelper = function (name, value) {
  globalThis.__hostrun_definePrototypeHelper(String.prototype, name, value);
};

globalThis.__hostrun_defineObjectHelper = function (name, value) {
  globalThis.__hostrun_definePrototypeHelper(Object.prototype, name, value);
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

globalThis.__hostrun_utf8Bytes = function (value) {
  const output = [];
  for (const char of String(value)) {
    let codePoint = char.codePointAt(0);
    if (codePoint <= 0x7f) {
      output.push(codePoint);
    } else if (codePoint <= 0x7ff) {
      output.push(0xc0 | (codePoint >> 6));
      output.push(0x80 | (codePoint & 0x3f));
    } else if (codePoint <= 0xffff) {
      output.push(0xe0 | (codePoint >> 12));
      output.push(0x80 | ((codePoint >> 6) & 0x3f));
      output.push(0x80 | (codePoint & 0x3f));
    } else {
      output.push(0xf0 | (codePoint >> 18));
      output.push(0x80 | ((codePoint >> 12) & 0x3f));
      output.push(0x80 | ((codePoint >> 6) & 0x3f));
      output.push(0x80 | (codePoint & 0x3f));
    }
  }
  return output;
};

globalThis.__hostrun_byteRange = function (values, start, end = start) {
  const first = Math.max(Number(start), 0);
  const last = Math.max(Number(end), first);
  return Array.from(values).slice(first, last + 1);
};

globalThis.__hostrun_lineRange = function (values, start, end = start) {
  if (start === undefined || start === null) {
    return values;
  }
  const first = Math.max(Number(start), 1) - 1;
  const last = Math.max(Number(end), Number(start));
  return values.slice(first, last);
};

globalThis.__hostrun_defineStringHelper("lines", function (start, end = start) {
  const text = String(this);
  if (text.length === 0) {
    return [];
  }
  const lines = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n");
  return globalThis.__hostrun_lineRange(lines, start, end);
});

globalThis.__hostrun_defineStringHelper("lineCount", function () {
  return String(this).lines().length;
});

globalThis.__hostrun_defineStringHelper("head", function (count = 10) {
  return String(this).lines().slice(0, Number(count));
});

globalThis.__hostrun_defineStringHelper("tail", function (count = 10) {
  return String(this).lines().slice(-Number(count));
});

globalThis.__hostrun_defineStringHelper("splitRow", function (separator = "\n") {
  return String(this).split(separator);
});

globalThis.__hostrun_defineStringHelper("splitWords", function () {
  const text = String(this).trim();
  return text.length === 0 ? [] : text.split(/\s+/);
});

globalThis.__hostrun_defineStringHelper("splitColumn", function (separator = /\s+/, names = null) {
  const rows = String(this).lines()
    .filter((line) => line.trim().length > 0)
    .map((line) => line.trim().split(separator).filter((field) => field.length > 0));
  if (names === null || names === undefined) {
    return rows;
  }
  return rows.map((row) => {
    const output = {};
    Array.from(names).forEach((name, index) => {
      output[name] = row[index] ?? null;
    });
    return output;
  });
});

globalThis.__hostrun_defineStringHelper("trimmed", function () {
  return String(this).trim();
});

globalThis.__hostrun_defineStringHelper("replaceText", function (from, to = "") {
  return String(this).replaceAll(from, to);
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

globalThis.__hostrun_defineStringHelper("byteArray", function () {
  return globalThis.__hostrun_utf8Bytes(this);
});

globalThis.__hostrun_defineStringHelper("byteRange", function (start, end = start) {
  return globalThis.__hostrun_byteRange(globalThis.__hostrun_utf8Bytes(this), start, end);
});

globalThis.__hostrun_defineStringHelper("chars", function () {
  return Array.from(String(this));
});

globalThis.__hostrun_pathParts = function (path) {
  return String(path).split(".").filter((part) => part.length > 0);
};

globalThis.__hostrun_pathValue = function (value, path) {
  let current = value;
  for (const part of globalThis.__hostrun_pathParts(path)) {
    if (current === null || current === undefined) {
      return undefined;
    }
    current = current[part];
  }
  return current;
};

globalThis.__hostrun_objectEntries = function (record) {
  return Object.entries(Object(record));
};

globalThis.__hostrun_objectFromFields = function (record, fields) {
  const output = {};
  for (const field of fields) {
    output[field] = globalThis.__hostrun_pathValue(record, field);
  }
  return output;
};

globalThis.__hostrun_objectWithoutFields = function (record, fields) {
  const rejected = new Set(fields.map((field) => String(field)));
  const output = {};
  for (const [key, value] of globalThis.__hostrun_objectEntries(record)) {
    if (!rejected.has(key)) {
      output[key] = value;
    }
  }
  return output;
};

globalThis.__hostrun_recordMatches = function (record, expected) {
  for (const [path, value] of globalThis.__hostrun_objectEntries(expected)) {
    if (globalThis.__hostrun_pathValue(record, path) !== value) {
      return false;
    }
  }
  return true;
};

globalThis.__hostrun_recordColumns = function (record) {
  return Object.keys(Object(record));
};

globalThis.__hostrun_tableColumns = function (rows) {
  const columns = [];
  const seen = new Set();
  for (const row of rows) {
    for (const key of Object.keys(Object(row))) {
      if (!seen.has(key)) {
        seen.add(key);
        columns.push(key);
      }
    }
  }
  return columns;
};

globalThis.__hostrun_recordRename = function (record, names) {
  const output = {};
  for (const [key, value] of globalThis.__hostrun_objectEntries(record)) {
    output[names[key] ?? key] = value;
  }
  return output;
};

globalThis.__hostrun_recordInsert = function (record, key, value) {
  return { ...Object(record), [key]: value };
};

globalThis.__hostrun_recordUpdate = function (record, key, valueOrFn) {
  const current = globalThis.__hostrun_pathValue(record, key);
  const next = typeof valueOrFn === "function" ? valueOrFn(current, record) : valueOrFn;
  return { ...Object(record), [key]: next };
};

globalThis.__hostrun_cleanValues = function (values) {
  return values.filter((value) => value !== null && value !== undefined && value !== "");
};

globalThis.__hostrun_numberValues = function (values) {
  return globalThis.__hostrun_cleanValues(values).map(Number).filter((value) => !Number.isNaN(value));
};

globalThis.__hostrun_transpose = function (rows) {
  const width = Math.max(0, ...rows.map((row) => Array.from(row).length));
  const output = [];
  for (let column = 0; column < width; column += 1) {
    output.push(rows.map((row) => row[column] ?? null));
  }
  return output;
};

globalThis.__hostrun_pathCleanParts = function (path) {
  return String(path).split("/").filter((part) => part.length > 0);
};

globalThis.__hostrun_pathBasename = function (path) {
  const parts = globalThis.__hostrun_pathCleanParts(path);
  return parts.length === 0 ? "" : parts[parts.length - 1];
};

globalThis.__hostrun_pathDirname = function (path) {
  const text = String(path);
  const absolute = text.startsWith("/");
  const parts = globalThis.__hostrun_pathCleanParts(text);
  parts.pop();
  if (parts.length === 0) {
    return absolute ? "/" : ".";
  }
  return (absolute ? "/" : "") + parts.join("/");
};

globalThis.__hostrun_pathParse = function (path) {
  const text = String(path);
  const dir = globalThis.__hostrun_pathDirname(text);
  const base = globalThis.__hostrun_pathBasename(text);
  const dot = base.lastIndexOf(".");
  const hasExtension = dot > 0;
  return {
    root: text.startsWith("/") ? "/" : "",
    dir,
    base,
    name: hasExtension ? base.slice(0, dot) : base,
    ext: hasExtension ? base.slice(dot) : ""
  };
};

globalThis.path = {
  join: function (...parts) {
    const absolute = parts.length > 0 && String(parts[0]).startsWith("/");
    const joined = parts.flat().map(String).join("/");
    const cleaned = globalThis.__hostrun_pathCleanParts(joined).join("/");
    return (absolute ? "/" : "") + cleaned;
  },
  basename: globalThis.__hostrun_pathBasename,
  dirname: globalThis.__hostrun_pathDirname,
  parse: globalThis.__hostrun_pathParse
};

globalThis.__hostrun_regex = function (pattern) {
  return pattern instanceof RegExp ? pattern : new RegExp(String(pattern));
};

globalThis.__hostrun_escapeRegex = function (value) {
  return String(value).replace(/[\\^$+?.()|[\]{}]/g, "\\$&");
};

globalThis.__hostrun_globRegex = function (pattern) {
  const glob = String(pattern);
  let source = "^";
  for (let index = 0; index < glob.length; index += 1) {
    const char = glob[index];
    if (char === "*") {
      const isGlobstar = glob[index + 1] === "*";
      if (isGlobstar && glob[index + 2] === "/") {
        source += "(?:.*\\/)?";
        index += 2;
      } else if (isGlobstar) {
        source += ".*";
        index += 1;
      } else {
        source += "[^/]*";
      }
    } else if (char === "?") {
      source += "[^/]";
    } else {
      source += globalThis.__hostrun_escapeRegex(char);
    }
  }
  return new RegExp(source + "$");
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
      return globalThis.path.basename(text);
    case "dirname":
      return globalThis.path.dirname(text);
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

globalThis.__hostrun_fieldSelector = function (fieldOrTemplate) {
  if (typeof fieldOrTemplate === "number" || /^\d+$/.test(String(fieldOrTemplate))) {
    const index = Number(fieldOrTemplate) - 1;
    return function (row) {
      return row[index] ?? "";
    };
  }
  return function (row) {
    return globalThis.__hostrun_formatTemplate(fieldOrTemplate, row);
  };
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
    },
    sortBy: function (fieldOrTemplate) {
      const selector = globalThis.__hostrun_fieldSelector(fieldOrTemplate);
      return globalThis.__hostrun_fieldTable(
        Array.from(rows).sort((left, right) => String(selector(left)).localeCompare(String(selector(right))))
      );
    },
    uniqueBy: function (fieldOrTemplate) {
      const selector = globalThis.__hostrun_fieldSelector(fieldOrTemplate);
      const seen = new Set();
      return globalThis.__hostrun_fieldTable(rows.filter((row) => {
        const key = String(selector(row));
        if (seen.has(key)) {
          return false;
        }
        seen.add(key);
        return true;
      }));
    },
    countBy: function (fieldOrTemplate) {
      const selector = globalThis.__hostrun_fieldSelector(fieldOrTemplate);
      const groups = [];
      const byKey = new Map();
      for (const row of rows) {
        const key = String(selector(row));
        let group = byKey.get(key);
        if (!group) {
          group = { key, count: 0 };
          byKey.set(key, group);
          groups.push(group);
        }
        group.count += 1;
      }
      return groups;
    },
    groupBy: function (fieldOrTemplate) {
      const selector = globalThis.__hostrun_fieldSelector(fieldOrTemplate);
      const groups = [];
      const byKey = new Map();
      for (const row of rows) {
        const key = String(selector(row));
        let group = byKey.get(key);
        if (!group) {
          group = { key, rows: [] };
          byKey.set(key, group);
          groups.push(group);
        }
        group.rows.push(row);
      }
      return groups;
    }
  };
};

globalThis.__hostrun_defineArrayHelper("fields", function (separator = /\s+/) {
  return globalThis.__hostrun_fieldTable(
    this.map((line) => String(line).trim().split(separator).filter((field) => field.length > 0))
  );
});

globalThis.__hostrun_defineArrayHelper("get", function (path) {
  return this.map((record) => globalThis.__hostrun_pathValue(record, path));
});

globalThis.__hostrun_defineArrayHelper("valuesOf", function (path) {
  return this.get(path);
});

globalThis.__hostrun_defineArrayHelper("pluck", function (path) {
  return this.get(path);
});

globalThis.__hostrun_defineArrayHelper("select", function (...fields) {
  return this.map((record) => globalThis.__hostrun_objectFromFields(record, fields));
});

globalThis.__hostrun_defineArrayHelper("reject", function (...fields) {
  return this.map((record) => globalThis.__hostrun_objectWithoutFields(record, fields));
});

globalThis.__hostrun_defineArrayHelper("where", function (predicateOrObject) {
  if (typeof predicateOrObject === "function") {
    return this.filter((record, index) => predicateOrObject(record, index));
  }
  return this.filter((record) => globalThis.__hostrun_recordMatches(record, predicateOrObject));
});

globalThis.__hostrun_defineArrayHelper("columns", function () {
  return globalThis.__hostrun_tableColumns(this);
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

globalThis.__hostrun_defineArrayHelper("glob", function (pattern) {
  const regex = globalThis.__hostrun_globRegex(pattern);
  return this.filter((value) => regex.test(String(value)));
});

globalThis.__hostrun_defineArrayHelper("notGlob", function (pattern) {
  const regex = globalThis.__hostrun_globRegex(pattern);
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

globalThis.__hostrun_defineArrayHelper("lineRange", function (start, end = start) {
  return globalThis.__hostrun_lineRange(this, start, end);
});

globalThis.__hostrun_defineArrayHelper("head", function (count = 10) {
  return this.slice(0, Number(count));
});

globalThis.__hostrun_defineArrayHelper("tail", function (count = 10) {
  return this.slice(-Number(count));
});

globalThis.__hostrun_defineArrayHelper("joinText", function (separator = "") {
  return this.join(separator);
});

globalThis.__hostrun_defineArrayHelper("unique", function () {
  return Array.from(new Set(this));
});

globalThis.__hostrun_defineArrayHelper("flatten", function (depth = 1) {
  return Array.from(this).flat(Number(depth));
});

globalThis.__hostrun_defineArrayHelper("compact", function () {
  return globalThis.__hostrun_cleanValues(this);
});

globalThis.__hostrun_defineArrayHelper("default", function (value) {
  return this.map((item) => item === null || item === undefined || item === "" ? value : item);
});

globalThis.__hostrun_defineArrayHelper("wrap", function (name) {
  return this.map((value) => ({ [name]: value }));
});

globalThis.__hostrun_defineArrayHelper("transpose", function () {
  return globalThis.__hostrun_transpose(this);
});

globalThis.__hostrun_defineArrayHelper("enumerate", function () {
  return this.map((item, index) => ({ index, item }));
});

globalThis.__hostrun_defineArrayHelper("isEmpty", function () {
  return this.length === 0;
});

globalThis.__hostrun_defineArrayHelper("isNotEmpty", function () {
  return this.length > 0;
});

globalThis.__hostrun_defineArrayHelper("any", function (predicate) {
  if (typeof predicate === "function") {
    return this.some((item, index) => predicate(item, index));
  }
  if (predicate !== undefined) {
    return this.some((item) => item === predicate);
  }
  return this.some(Boolean);
});

globalThis.__hostrun_defineArrayHelper("all", function (predicate) {
  if (typeof predicate === "function") {
    return this.every((item, index) => predicate(item, index));
  }
  if (predicate !== undefined) {
    return this.every((item) => item === predicate);
  }
  return this.every(Boolean);
});

globalThis.__hostrun_defineArrayHelper("sum", function () {
  return globalThis.__hostrun_numberValues(this).reduce((total, value) => total + value, 0);
});

globalThis.__hostrun_defineArrayHelper("avg", function () {
  const values = globalThis.__hostrun_numberValues(this);
  return values.length === 0 ? null : values.sum() / values.length;
});

globalThis.__hostrun_defineArrayHelper("min", function () {
  const values = globalThis.__hostrun_numberValues(this);
  return values.length === 0 ? null : Math.min(...values);
});

globalThis.__hostrun_defineArrayHelper("max", function () {
  const values = globalThis.__hostrun_numberValues(this);
  return values.length === 0 ? null : Math.max(...values);
});

globalThis.__hostrun_defineArrayHelper("round", function (digits = 0) {
  const factor = 10 ** Number(digits);
  return this.map((value) => {
    const number = globalThis.__hostrun_numberValues([value])[0];
    return number === undefined ? null : Math.round(number * factor) / factor;
  });
});

globalThis.__hostrun_defineArrayHelper("lengths", function () {
  return this.map((value) => String(value).length);
});

globalThis.__hostrun_defineArrayHelper("bytes", function () {
  return this.map((value) => String(value).bytes());
});

globalThis.__hostrun_defineArrayHelper("byteRange", function (start, end = start) {
  return globalThis.__hostrun_byteRange(this, start, end);
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

globalThis.__hostrun_httpRequestBuilder = function (method, url, options = {}) {
  const bodySources = ["json", "form", "body", "file", "multipart"].filter((key) => options[key] !== undefined);
  if (bodySources.length > 1) {
    throw new Error(`http request has multiple body sources: ${bodySources.join(", ")}`);
  }
  const state = {
    method: String(method).toUpperCase(),
    url: String(url),
    ...options
  };
  const builder = {
    run: function () {
      return globalThis.__hostrun_invokeCapability("http.request", state);
    },
    text: function () {
      state.response = { type: "text" };
      return this.run();
    },
    json: function () {
      state.response = { type: "json" };
      return this.run();
    },
    bytes: function () {
      state.response = { type: "bytes" };
      return this.run();
    },
    save: function (path) {
      state.response = { type: "file", path };
      return this.run();
    },
    toJSON: function () {
      return { ...state };
    }
  };
  return builder;
};

globalThis.http = {
  request: function (method, url, options = {}) {
    return globalThis.__hostrun_httpRequestBuilder(method, url, options);
  },
  get: function (url, options = {}) {
    return globalThis.http.request("GET", url, options);
  },
  post: function (url, options = {}) {
    return globalThis.http.request("POST", url, options);
  },
  put: function (url, options = {}) {
    return globalThis.http.request("PUT", url, options);
  },
  patch: function (url, options = {}) {
    return globalThis.http.request("PATCH", url, options);
  },
  delete: function (url, options = {}) {
    return globalThis.http.request("DELETE", url, options);
  },
  head: function (url, options = {}) {
    return globalThis.http.request("HEAD", url, options);
  }
};

globalThis.__hostrun_defineObjectHelper("get", function (path) {
  return globalThis.__hostrun_pathValue(this, path);
});

globalThis.__hostrun_defineObjectHelper("select", function (...fields) {
  return globalThis.__hostrun_objectFromFields(this, fields);
});

globalThis.__hostrun_defineObjectHelper("reject", function (...fields) {
  return globalThis.__hostrun_objectWithoutFields(this, fields);
});

globalThis.__hostrun_defineObjectHelper("rename", function (names) {
  return globalThis.__hostrun_recordRename(this, names);
});

globalThis.__hostrun_defineObjectHelper("insert", function (key, value) {
  return globalThis.__hostrun_recordInsert(this, key, value);
});

globalThis.__hostrun_defineObjectHelper("update", function (key, valueOrFn) {
  return globalThis.__hostrun_recordUpdate(this, key, valueOrFn);
});

globalThis.__hostrun_defineObjectHelper("merge", function (other) {
  return { ...Object(this), ...Object(other) };
});

globalThis.__hostrun_defineObjectHelper("columns", function () {
  return globalThis.__hostrun_recordColumns(this);
});

globalThis.__hostrun_defineObjectHelper("values", function () {
  return Object.values(Object(this));
});

globalThis.__hostrun_defineObjectHelper("entries", function () {
  return globalThis.__hostrun_objectEntries(this);
});

globalThis.__hostrun_defineObjectHelper("items", function () {
  return globalThis.__hostrun_objectEntries(this);
});

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
