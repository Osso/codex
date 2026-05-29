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

globalThis.__hostrun_uintFromBytes = function (values, offset, length, littleEndian) {
  const bytes = Array.from(values);
  const start = Number(offset) || 0;
  let value = 0;
  for (let index = 0; index < Number(length); index += 1) {
    const byte = Number(bytes[start + index] ?? 0) & 0xff;
    const shift = littleEndian ? index * 8 : (Number(length) - index - 1) * 8;
    value += byte * (2 ** shift);
  }
  return value;
};

globalThis.__hostrun_intFromBytes = function (values, offset, length, littleEndian) {
  const unsigned = globalThis.__hostrun_uintFromBytes(values, offset, length, littleEndian);
  const sign = 2 ** (Number(length) * 8 - 1);
  return unsigned >= sign ? unsigned - (sign * 2) : unsigned;
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

globalThis.__hostrun_defineStringHelper("wordCount", function () {
  return String(this).splitWords().length;
});

globalThis.__hostrun_defineStringHelper("byteCount", function () {
  return String(this).bytes();
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

globalThis.__hostrun_defineStringHelper("cut", function (separator = /\s+/, fields = []) {
  const indexes = Array.from(fields).map((field) => Number(field) - 1);
  return String(this).splitColumn(separator).map((row) => indexes.map((index) => row[index] ?? ""));
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

globalThis.__hostrun_defineStringHelper("jsonl", function () {
  return String(this).jsonLines();
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

globalThis.__hostrun_collectionSelector = function (selector) {
  if (typeof selector === "function") {
    return selector;
  }
  if (selector === undefined || selector === null) {
    return function (item) {
      return item;
    };
  }
  return function (item) {
    if (Array.isArray(item)) {
      return globalThis.__hostrun_formatTemplate(selector, item);
    }
    const value = globalThis.__hostrun_pathValue(item, selector);
    return value === undefined ? "" : value;
  };
};

globalThis.__hostrun_groupValues = function (values, selector) {
  const select = globalThis.__hostrun_collectionSelector(selector);
  const groups = [];
  const byKey = new Map();
  for (const item of values) {
    const key = String(select(item));
    let group = byKey.get(key);
    if (!group) {
      group = { key, rows: [] };
      byKey.set(key, group);
      groups.push(group);
    }
    group.rows.push(item);
  }
  return groups;
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

globalThis.__hostrun_formatFromPath = function (path) {
  switch (globalThis.path.parse(path).ext.toLowerCase()) {
    case ".json":
      return "json";
    case ".jsonl":
    case ".ndjson":
      return "jsonl";
    case ".yaml":
    case ".yml":
      return "yaml";
    case ".toml":
      return "toml";
    case ".csv":
      return "csv";
    case ".tsv":
      return "tsv";
    default:
      return "text";
  }
};

globalThis.__hostrun_parseTextFormat = function (text, format) {
  switch (String(format ?? "text").toLowerCase()) {
    case "json":
      return String(text).json();
    case "jsonl":
    case "jsonlines":
    case "ndjson":
      return String(text).jsonLines();
    case "yaml":
    case "yml":
      return String(text).yaml();
    case "toml":
      return String(text).toml();
    case "csv":
      return String(text).csv();
    case "tsv":
      return String(text).tsv();
    case "text":
    case "raw":
      return String(text);
    default:
      throw new Error("unknown Hostrun file format: " + format);
  }
};

globalThis.__hostrun_padDate = function (value, width = 2) {
  return String(value).padStart(width, "0");
};

globalThis.__hostrun_formatDate = function (value, template = "YYYY-MM-DDTHH:mm:ssZ") {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error("invalid Hostrun date: " + value);
  }
  return String(template)
    .replaceAll("YYYY", globalThis.__hostrun_padDate(date.getUTCFullYear(), 4))
    .replaceAll("MM", globalThis.__hostrun_padDate(date.getUTCMonth() + 1))
    .replaceAll("DD", globalThis.__hostrun_padDate(date.getUTCDate()))
    .replaceAll("HH", globalThis.__hostrun_padDate(date.getUTCHours()))
    .replaceAll("mm", globalThis.__hostrun_padDate(date.getUTCMinutes()))
    .replaceAll("ss", globalThis.__hostrun_padDate(date.getUTCSeconds()))
    .replaceAll("Z", "Z");
};

globalThis.__hostrun_humanizeDuration = function (milliseconds) {
  const seconds = Math.round(Math.abs(Number(milliseconds)) / 1000);
  const units = [
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
    ["second", 1]
  ];
  for (const [name, size] of units) {
    if (seconds >= size || name === "second") {
      const count = Math.floor(seconds / size);
      return `${count} ${name}${count === 1 ? "" : "s"}`;
    }
  }
};

globalThis.date = {
  now: function () {
    return new Date().toISOString();
  },
  parse: function (value) {
    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
      throw new Error("invalid Hostrun date: " + value);
    }
    return parsed;
  },
  format: globalThis.__hostrun_formatDate,
  humanize: function (value, base = new Date()) {
    const target = value instanceof Date ? value : new Date(value);
    const origin = base instanceof Date ? base : new Date(base);
    const suffix = target.getTime() >= origin.getTime() ? "from now" : "ago";
    return `${globalThis.__hostrun_humanizeDuration(target.getTime() - origin.getTime())} ${suffix}`;
  }
};

globalThis.__hostrun_csvCell = function (value) {
  const text = value === null || value === undefined ? "" : String(value);
  return /[",\n\r]/.test(text) ? '"' + text.replaceAll('"', '""') + '"' : text;
};

globalThis.__hostrun_toCsv = function (rows) {
  return rows.map((row) => Array.from(row).map(globalThis.__hostrun_csvCell).join(",")).join("\n") + "\n";
};

globalThis.__hostrun_tsvCell = function (value) {
  const text = value === null || value === undefined ? "" : String(value);
  return text.replaceAll("\\", "\\\\").replaceAll("\t", "\\t").replaceAll("\n", "\\n").replaceAll("\r", "\\r");
};

globalThis.__hostrun_toTsv = function (rows) {
  return rows.map((row) => Array.from(row).map(globalThis.__hostrun_tsvCell).join("\t")).join("\n") + "\n";
};

globalThis.__hostrun_toJsonLines = function (values) {
  return Array.from(values).map((value) => JSON.stringify(value)).join("\n") + "\n";
};

globalThis.__hostrun_parseCsv = function (text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  const input = String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];
    if (quoted) {
      if (char === '"' && input[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        cell += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === ",") {
      row.push(cell);
      cell = "";
    } else if (char === "\n") {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else {
      cell += char;
    }
  }
  if (cell.length > 0 || row.length > 0) {
    row.push(cell);
    rows.push(row);
  }
  return rows;
};

globalThis.__hostrun_parseTsvCell = function (value) {
  return String(value)
    .replaceAll("\\t", "\t")
    .replaceAll("\\n", "\n")
    .replaceAll("\\r", "\r")
    .replaceAll("\\\\", "\\");
};

globalThis.__hostrun_parseTsv = function (text) {
  return String(text).lines()
    .filter((line) => line.length > 0)
    .map((line) => line.split("\t").map(globalThis.__hostrun_parseTsvCell));
};

globalThis.__hostrun_defineStringHelper("csv", function () {
  return globalThis.__hostrun_parseCsv(this);
});

globalThis.__hostrun_defineStringHelper("tsv", function () {
  return globalThis.__hostrun_parseTsv(this);
});

globalThis.__hostrun_toYamlScalar = function (value) {
  if (value === null || value === undefined) {
    return "null";
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(String(value));
};

globalThis.__hostrun_toYaml = function (value, indent = 0) {
  const prefix = " ".repeat(indent);
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return "[]";
    }
    return value.map((item) => {
      if (item !== null && typeof item === "object") {
        return prefix + "-\n" + globalThis.__hostrun_toYaml(item, indent + 2);
      }
      return prefix + "- " + globalThis.__hostrun_toYamlScalar(item);
    }).join("\n");
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return "{}";
    }
    return entries.map(([key, item]) => {
      if (item !== null && typeof item === "object") {
        return prefix + key + ":\n" + globalThis.__hostrun_toYaml(item, indent + 2);
      }
      return prefix + key + ": " + globalThis.__hostrun_toYamlScalar(item);
    }).join("\n");
  }
  return prefix + globalThis.__hostrun_toYamlScalar(value);
};

globalThis.__hostrun_parseYamlScalar = function (value) {
  const text = String(value).trim();
  if (text === "null" || text === "~") {
    return null;
  }
  if (text === "true") {
    return true;
  }
  if (text === "false") {
    return false;
  }
  if (text === "[]") {
    return [];
  }
  if (text === "{}") {
    return {};
  }
  if (/^-?\d+(\.\d+)?$/.test(text)) {
    return Number(text);
  }
  if ((text.startsWith('"') && text.endsWith('"')) || (text.startsWith("'") && text.endsWith("'"))) {
    return JSON.parse(text.startsWith("'") ? JSON.stringify(text.slice(1, -1)) : text);
  }
  return text;
};

globalThis.__hostrun_yamlItems = function (text) {
  return String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => ({
      indent: line.match(/^ */)[0].length,
      text: line.trim()
    }));
};

globalThis.__hostrun_parseYamlBlock = function (items, start = 0, indent = 0) {
  if (start >= items.length) {
    return [null, start];
  }
  if (items[start].indent < indent) {
    return [null, start];
  }
  if (items[start].text.startsWith("-")) {
    return globalThis.__hostrun_parseYamlArray(items, start, indent);
  }
  return globalThis.__hostrun_parseYamlObject(items, start, indent);
};

globalThis.__hostrun_parseYamlArray = function (items, start, indent) {
  const values = [];
  let index = start;
  while (index < items.length && items[index].indent === indent && items[index].text.startsWith("-")) {
    const rest = items[index].text.slice(1).trim();
    if (rest.length === 0) {
      const [value, next] = globalThis.__hostrun_parseYamlBlock(items, index + 1, indent + 2);
      values.push(value);
      index = next;
    } else {
      values.push(globalThis.__hostrun_parseYamlScalar(rest));
      index += 1;
    }
  }
  return [values, index];
};

globalThis.__hostrun_parseYamlObject = function (items, start, indent) {
  const record = {};
  let index = start;
  while (index < items.length && items[index].indent === indent && !items[index].text.startsWith("-")) {
    const [key, rest = ""] = items[index].text.split(/:(.*)/s);
    if (rest.trim().length === 0) {
      const [value, next] = globalThis.__hostrun_parseYamlBlock(items, index + 1, indent + 2);
      record[key] = value;
      index = next;
    } else {
      record[key] = globalThis.__hostrun_parseYamlScalar(rest);
      index += 1;
    }
  }
  return [record, index];
};

globalThis.__hostrun_parseYaml = function (text) {
  const items = globalThis.__hostrun_yamlItems(text);
  if (items.length === 0) {
    return null;
  }
  if (items.length === 1 && !items[0].text.includes(":") && !items[0].text.startsWith("-")) {
    return globalThis.__hostrun_parseYamlScalar(items[0].text);
  }
  return globalThis.__hostrun_parseYamlBlock(items, 0, items[0].indent)[0];
};

globalThis.__hostrun_defineStringHelper("yaml", function () {
  return globalThis.__hostrun_parseYaml(this);
});

globalThis.__hostrun_parseTomlValue = function (value) {
  const text = String(value).trim();
  if (text === "true") {
    return true;
  }
  if (text === "false") {
    return false;
  }
  if (/^-?\d+(\.\d+)?$/.test(text)) {
    return Number(text);
  }
  if (text.startsWith("[") && text.endsWith("]")) {
    const jsonText = "[" + text.slice(1, -1).split(",").map((item) => item.trim()).join(",") + "]";
    return JSON.parse(jsonText);
  }
  if (text.startsWith('"') && text.endsWith('"')) {
    return JSON.parse(text);
  }
  return text;
};

globalThis.__hostrun_parseToml = function (text) {
  const root = {};
  let current = root;
  for (const rawLine of String(text).lines()) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith("#")) {
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      current = root;
      for (const part of line.slice(1, -1).split(".")) {
        current[part] = current[part] ?? {};
        current = current[part];
      }
      continue;
    }
    const match = line.match(/^([A-Za-z0-9_.-]+)\s*=\s*(.*)$/);
    if (!match) {
      throw new Error("unsupported Hostrun TOML line: " + line);
    }
    current[match[1]] = globalThis.__hostrun_parseTomlValue(match[2]);
  }
  return root;
};

globalThis.__hostrun_defineStringHelper("toml", function () {
  return globalThis.__hostrun_parseToml(this);
});

globalThis.__hostrun_defineObjectHelper("toJson", function (space = 0) {
  return JSON.stringify(this, null, Number(space));
});

globalThis.__hostrun_defineObjectHelper("toYaml", function () {
  return globalThis.__hostrun_toYaml(this) + "\n";
});

globalThis.__hostrun_toTomlValue = function (value) {
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return "[" + value.map(globalThis.__hostrun_toTomlValue).join(", ") + "]";
  }
  if (value === null || value === undefined) {
    return '""';
  }
  return JSON.stringify(value);
};

globalThis.__hostrun_toToml = function (record) {
  return Object.entries(Object(record))
    .map(([key, value]) => key + " = " + globalThis.__hostrun_toTomlValue(value))
    .join("\n") + "\n";
};

globalThis.__hostrun_defineObjectHelper("toToml", function () {
  return globalThis.__hostrun_toToml(this);
});

globalThis.__hostrun_markdownCell = function (value) {
  return String(value ?? "").replaceAll("|", "\\|").replace(/\r?\n/g, "<br>");
};

globalThis.__hostrun_toMarkdown = function (rows) {
  const values = Array.from(rows);
  if (values.length === 0) {
    return "";
  }
  const columns = globalThis.__hostrun_tableColumns(values);
  const header = "| " + columns.map(globalThis.__hostrun_markdownCell).join(" | ") + " |";
  const separator = "| " + columns.map(() => "---").join(" | ") + " |";
  const body = values.map((row) => {
    const cells = columns.map((column) => globalThis.__hostrun_markdownCell(row[column]));
    return "| " + cells.join(" | ") + " |";
  });
  return [header, separator, ...body].join("\n") + "\n";
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

globalThis.__hostrun_defineArrayHelper("toCsv", function () {
  return globalThis.__hostrun_toCsv(this);
});

globalThis.__hostrun_defineArrayHelper("toTsv", function () {
  return globalThis.__hostrun_toTsv(this);
});

globalThis.__hostrun_defineArrayHelper("toMarkdown", function () {
  return globalThis.__hostrun_toMarkdown(this);
});

globalThis.__hostrun_defineArrayHelper("toMd", function () {
  return globalThis.__hostrun_toMarkdown(this);
});

globalThis.__hostrun_defineArrayHelper("toJsonLines", function () {
  return globalThis.__hostrun_toJsonLines(this);
});

globalThis.__hostrun_defineArrayHelper("toJsonl", function () {
  return globalThis.__hostrun_toJsonLines(this);
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

globalThis.__hostrun_defineArrayHelper("groupBy", function (selector) {
  return globalThis.__hostrun_groupValues(this, selector);
});

globalThis.__hostrun_defineArrayHelper("countBy", function (selector) {
  return globalThis.__hostrun_groupValues(this, selector).map((group) => ({
    key: group.key,
    count: group.rows.length
  }));
});

globalThis.__hostrun_defineArrayHelper("uniqueBy", function (selector) {
  return globalThis.__hostrun_groupValues(this, selector).map((group) => group.rows[0]);
});

globalThis.__hostrun_defineArrayHelper("sortBy", function (selector) {
  const select = globalThis.__hostrun_collectionSelector(selector);
  return Array.from(this).sort((left, right) => String(select(left)).localeCompare(String(select(right))));
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

globalThis.__hostrun_defineArrayHelper("u16le", function (offset = 0) {
  return globalThis.__hostrun_uintFromBytes(this, offset, 2, true);
});

globalThis.__hostrun_defineArrayHelper("u16be", function (offset = 0) {
  return globalThis.__hostrun_uintFromBytes(this, offset, 2, false);
});

globalThis.__hostrun_defineArrayHelper("u32le", function (offset = 0) {
  return globalThis.__hostrun_uintFromBytes(this, offset, 4, true);
});

globalThis.__hostrun_defineArrayHelper("u32be", function (offset = 0) {
  return globalThis.__hostrun_uintFromBytes(this, offset, 4, false);
});

globalThis.__hostrun_defineArrayHelper("i32le", function (offset = 0) {
  return globalThis.__hostrun_intFromBytes(this, offset, 4, true);
});

globalThis.__hostrun_defineArrayHelper("i32be", function (offset = 0) {
  return globalThis.__hostrun_intFromBytes(this, offset, 4, false);
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

globalThis.__hostrun_tmpCounter = globalThis.__hostrun_tmpCounter ?? 0;
globalThis.__hostrun_tmpResources = globalThis.__hostrun_tmpResources ?? [];

globalThis.__hostrun_nextTmpPath = function (prefix, suffix = "") {
  globalThis.__hostrun_tmpCounter += 1;
  const cleanPrefix = String(prefix ?? "tmp").replace(/[^a-zA-Z0-9._-]/g, "-");
  const cleanSuffix = suffix ? String(suffix).replace(/[^a-zA-Z0-9._-]/g, "-") : "";
  return `/tmp/hostrun-${cleanPrefix}-${globalThis.__hostrun_tmpCounter}${cleanSuffix}`;
};

globalThis.__hostrun_tmpHandle = function (kind, path) {
  globalThis.__hostrun_tmpResources.push({ kind, path });
  const handle = {
    kind,
    path,
    toString: function () {
      return path;
    },
    cleanup: function () {
      return globalThis.fs.remove(path);
    },
    toJSON: function () {
      return { kind, path };
    }
  };
  if (kind === "file") {
    handle.write = function (content) {
      return globalThis.fs.write(path, content);
    };
    handle.writeJson = function (value, space = 2) {
      return globalThis.fs.writeJson(path, value, space);
    };
    handle.writeYaml = function (value) {
      return globalThis.fs.writeYaml(path, value);
    };
    handle.writeToml = function (value) {
      return globalThis.fs.writeToml(path, value);
    };
    handle.writeCsv = function (rows) {
      return globalThis.fs.writeCsv(path, rows);
    };
    handle.writeTsv = function (rows) {
      return globalThis.fs.writeTsv(path, rows);
    };
    handle.writeJsonLines = function (values) {
      return globalThis.fs.writeJsonLines(path, values);
    };
    handle.writeJsonl = function (values) {
      return globalThis.fs.writeJsonLines(path, values);
    };
  }
  return handle;
};

globalThis.__hostrun_toolProxy = function (path) {
  return new Proxy(function () {}, {
    get(target, property) {
      if (Reflect.has(target, property)) {
        return Reflect.get(target, property);
      }
      return globalThis.__hostrun_toolProxy(path ? path + "." + String(property) : String(property));
    },
    set(target, property, value) {
      Reflect.set(target, property, value);
      return true;
    },
    apply(_target, _thisArg, args) {
      const payload = args.length > 0 ? args[0] : {};
      return globalThis.__hostrun_invokeCapability(path, payload);
    }
  });
};

globalThis.tools = globalThis.__hostrun_toolProxy("");

globalThis.host = {
  cwd: function () {
    return globalThis.__hostrun_invokeCapability("host.cwd", {});
  },
  cd: function (path) {
    return globalThis.__hostrun_invokeCapability("host.cd", { path });
  }
};

globalThis.fs = {
  write: function (path, content) {
    return globalThis.__hostrun_invokeCapability("fs.write", { path, content });
  },
  writeJson: function (path, value, space = 2) {
    return globalThis.fs.write(path, JSON.stringify(value, null, space) + "\n");
  },
  writeYaml: function (path, value) {
    return globalThis.fs.write(path, globalThis.__hostrun_toYaml(value) + "\n");
  },
  writeToml: function (path, value) {
    return globalThis.fs.write(path, globalThis.__hostrun_toToml(value));
  },
  writeCsv: function (path, rows) {
    return globalThis.fs.write(path, globalThis.__hostrun_toCsv(rows));
  },
  writeTsv: function (path, rows) {
    return globalThis.fs.write(path, globalThis.__hostrun_toTsv(rows));
  },
  writeJsonLines: function (path, values) {
    return globalThis.fs.write(path, globalThis.__hostrun_toJsonLines(values));
  },
  writeJsonl: function (path, values) {
    return globalThis.fs.writeJsonLines(path, values);
  },
  read: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.read", { path });
  },
  open: function (path, options = {}) {
    const text = globalThis.fs.read(path);
    return globalThis.__hostrun_parseTextFormat(text, options.format ?? globalThis.__hostrun_formatFromPath(path));
  },
  glob: function (pattern, options = {}) {
    return globalThis.__hostrun_invokeCapability("fs.glob", { pattern, options });
  },
  exists: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.exists", { path });
  },
  remove: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.remove", { path });
  }
};

globalThis.tmp = {
  file: function (prefix = "tmp", options = {}) {
    return globalThis.__hostrun_tmpHandle("file", globalThis.__hostrun_nextTmpPath(prefix, options.suffix ?? ""));
  },
  dir: function (prefix = "tmp") {
    return globalThis.__hostrun_tmpHandle("dir", globalThis.__hostrun_nextTmpPath(prefix, ""));
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

globalThis.__hostrun_commandBuilder = function (program, args, options = {}) {
  const state = {
    program,
    args: Array.from(args),
    ...options
  };
  state.program = program;
  state.args = Array.from(args);
  const builder = {
    program: state.program,
    args: state.args,
    run: function () {
      return globalThis.__hostrun_invokeCapability("cli." + state.program, state);
    },
    spawn: function () {
      state.action = "spawn";
      return globalThis.__hostrun_processHandle(globalThis.__hostrun_invokeCapability("cli." + state.program, state));
    },
    in: function (path) {
      state.cwd = String(path);
      return builder;
    },
    toJSON: function () {
      return { ...state };
    }
  };
  const streamHandle = function (name) {
    const runWithParsedOutput = function (parser) {
      return globalThis.__hostrun_parseCommandOutput(builder.run(), name, parser);
    };
    return {
      stream: name,
      command: state,
      capture: function () {
        state[name] = { type: "capture" };
        return builder;
      },
      text: function () {
        state[name] = { type: "text" };
        return builder.run();
      },
      lines: function () {
        state[name] = { type: "lines" };
        return builder.run();
      },
      json: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => JSON.parse(text));
      },
      jsonLines: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => text.jsonLines());
      },
      jsonl: function () {
        return this.jsonLines();
      },
      csv: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => text.csv());
      },
      tsv: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => text.tsv());
      },
      yaml: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => text.yaml());
      },
      toml: function () {
        state[name] = { type: "text" };
        return runWithParsedOutput((text) => text.toml());
      },
      toFile: function (path) {
        state[name] = { type: "file", path };
        return builder;
      },
      tee: function (path) {
        state[name] = { type: "tee", path };
        return builder;
      },
      toJSON: function () {
        return { stream: name, command: { ...state } };
      }
    };
  };
  builder.stdout = streamHandle("stdout");
  builder.stderr = streamHandle("stderr");
  builder.text = function () {
    return builder.stdout.text().stdout ?? "";
  };
  builder.lines = function () {
    return builder.stdout.lines().stdout ?? [];
  };
  for (const method of ["json", "jsonLines", "jsonl", "csv", "tsv", "yaml", "toml"]) {
    builder[method] = function () {
      return builder.stdout[method]().stdout ?? null;
    };
  }
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
    },
    tee: function (path) {
      state.combined = { type: "tee", path };
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
  stdin.yaml = function (value) {
    state.stdin = { type: "yaml", value };
    return builder;
  };
  stdin.toml = function (value) {
    state.stdin = { type: "text", text: globalThis.__hostrun_toToml(value) };
    return builder;
  };
  stdin.csv = function (rows) {
    state.stdin = { type: "csv", rows };
    return builder;
  };
  stdin.tsv = function (rows) {
    state.stdin = { type: "tsv", rows };
    return builder;
  };
  stdin.jsonLines = function (values) {
    state.stdin = { type: "jsonLines", values };
    return builder;
  };
  stdin.jsonl = function (values) {
    state.stdin = { type: "jsonLines", values };
    return builder;
  };
  stdin.lines = function (lines) {
    state.stdin = { type: "lines", lines };
    return builder;
  };
  builder.stdin = stdin;
  return builder;
};

globalThis.__hostrun_processHandle = function (process) {
  return {
    id: process.id,
    pid: process.pid,
    program: process.program,
    args: process.args ?? [],
    stdout: process.stdout,
    stderr: process.stderr,
    wait: function () {
      return globalThis.__hostrun_invokeCapability("process.wait", { id: process.id });
    },
    kill: function () {
      return globalThis.__hostrun_invokeCapability("process.kill", { id: process.id });
    },
    toJSON: function () {
      return process;
    }
  };
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

globalThis.__hostrun_runProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_runProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      if (!path) {
        return {
          ok: false,
          error: "run is a program proxy, not a shell parser.",
          use: [
            "run.dmidecode('-t', 'system')",
            "cli.dmidecode('-t', 'system').stdout.text()",
            "tools.sudo(cli.dmidecode('-t', 'system')).run() for privileged commands"
          ],
          note: "cli.sudo(...) and run.sudo(...) invoke the sudo binary literally. tools.sudo(commandBuilder) wraps a cli.* builder with authsudo and captures stdout/stderr by default."
        };
      }
      return globalThis.__hostrun_commandBuilder(path, args).run();
    }
  });
};

globalThis.run = globalThis.__hostrun_runProxy("");

globalThis.tools.sudo = function (command) {
  if (!command || typeof command.toJSON !== "function") {
    throw new Error("tools.sudo expects a cli.* command builder, e.g. tools.sudo(cli.dmidecode('-t', 'system')).run()");
  }
  const state = command.toJSON();
  const { program, args = [], ...options } = state;
  if (!program) {
    throw new Error("tools.sudo command builder is missing a program");
  }
  const sudoOptions = { ...options };
  if (!("stdout" in sudoOptions) && !("combined" in sudoOptions)) {
    sudoOptions.stdout = { type: "text" };
  }
  if (!("stderr" in sudoOptions) && !("combined" in sudoOptions)) {
    sudoOptions.stderr = { type: "text" };
  }
  return globalThis.__hostrun_commandBuilder("authsudo", [program, ...args], sudoOptions);
};

globalThis.__hostrun_browserCommand = function (...args) {
  return globalThis.__hostrun_commandBuilder("browser-cli", args.flat().filter((arg) => arg !== undefined && arg !== null));
};

globalThis.__hostrun_browserJsonFlag = function (options) {
  return options?.json ? ["--json"] : [];
};

globalThis.__hostrun_browserSnapshotFlags = function (options = {}) {
  const args = [];
  if (options.react) args.push("--react");
  if (options.full) args.push("--full");
  if (options.mini) args.push("--mini");
  if (options.interactive) args.push("--interactive");
  if (options.compact) args.push("--compact");
  if (options.depth !== undefined) args.push("--depth", String(options.depth));
  if (options.filter !== undefined) args.push("--filter", String(options.filter));
  return args;
};

globalThis.__hostrun_browserScreenshotFlags = function (path, options = {}) {
  const args = [];
  if (options.full) args.push("--full");
  if (path !== undefined && path !== null) args.push(String(path));
  return args;
};

globalThis.browser = {
  command: function (...args) {
    return globalThis.__hostrun_browserCommand(...args);
  },
  open: function (url, options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "open", String(url));
  },
  goto: function (url, options = {}) {
    return this.open(url, options);
  },
  navigate: function (url, options = {}) {
    return this.open(url, options);
  },
  back: function (options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "back");
  },
  forward: function (options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "forward");
  },
  reload: function (options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "reload");
  },
  close: function (options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "close");
  },
  click: function (selector, options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "click", String(selector));
  },
  type: function (selector, text, options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "type", String(selector), String(text));
  },
  fill: function (selector, text, options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "fill", String(selector), String(text));
  },
  press: function (key, options = {}) {
    return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "press", String(key));
  },
  get: function (kind, ...args) {
    return globalThis.__hostrun_browserCommand("get", String(kind), ...args.map(String));
  },
  title: function () {
    return this.get("title").text();
  },
  url: function () {
    return this.get("url").text();
  },
  text: function (selector) {
    return selector === undefined ? this.get("text").text() : this.get("text", selector).text();
  },
  html: function (selector) {
    return this.get("html", selector).text();
  },
  value: function (selector) {
    return this.get("value", selector).text();
  },
  attr: function (selector, name) {
    return this.get("attr", selector, name).text();
  },
  count: function (selector) {
    return Number(this.get("count", selector).text());
  },
  screenshot: function (path, options = {}) {
    return globalThis.__hostrun_browserCommand("screenshot", ...globalThis.__hostrun_browserScreenshotFlags(path, options));
  },
  snapshot: function (options = {}) {
    return globalThis.__hostrun_browserCommand("snapshot", ...globalThis.__hostrun_browserSnapshotFlags(options));
  },
  wait: function (target, options = {}) {
    if (options.url !== undefined) return globalThis.__hostrun_browserCommand("wait", "--url", String(options.url));
    if (options.load !== undefined) return globalThis.__hostrun_browserCommand("wait", "--load", String(options.load));
    return globalThis.__hostrun_browserCommand("wait", String(target));
  },
  eval: function (code) {
    return globalThis.__hostrun_browserCommand("eval", String(code));
  },
  tabs: {
    list: function (options = {}) {
      return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "tabs", "list");
    },
    new: function (url, options = {}) {
      return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "tabs", "new", url === undefined ? undefined : String(url));
    },
    close: function (index, options = {}) {
      return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "tabs", "close", index === undefined ? undefined : String(index));
    },
    switch: function (index, options = {}) {
      return globalThis.__hostrun_browserCommand(...globalThis.__hostrun_browserJsonFlag(options), "tabs", "switch", String(index));
    }
  }
};

globalThis.tools.browser = globalThis.browser;

globalThis.which = function (program) {
  return globalThis.cli.which(String(program));
};

globalThis.__hostrun_values = function (value) {
  if (value === undefined || value === null || value === false) {
    return [];
  }
  return Array.isArray(value) ? value : [value];
};

globalThis.__hostrun_addOption = function (args, flag, value) {
  if (value === undefined || value === null || value === false) {
    return;
  }
  args.push(flag);
  if (value !== true) {
    args.push(String(value));
  }
};

globalThis.__hostrun_addRepeatedOption = function (args, flag, value) {
  for (const item of globalThis.__hostrun_values(value)) {
    globalThis.__hostrun_addOption(args, flag, item);
  }
};

globalThis.__hostrun_githubPrBody = function (options) {
  if (options.bodyLines !== undefined) {
    return Array.from(options.bodyLines).map((line) => String(line)).join("\n");
  }
  if (Array.isArray(options.body)) {
    return options.body.map((line) => String(line)).join("\n");
  }
  if (options.body !== undefined && options.body !== null) {
    return String(options.body);
  }
  return undefined;
};

globalThis.__hostrun_validateGithubPrBody = function (body, options) {
  if (body === undefined || options.allowEscapedNewlines === true) {
    return;
  }
  if (body.includes("\\n")) {
    throw new Error(
      "GitHub PR body contains literal \\\\n. Use a template literal, a normal newline string, or bodyLines instead."
    );
  }
};

globalThis.__hostrun_gitCommitBody = function (options) {
  if (options.bodyLines !== undefined) {
    return Array.from(options.bodyLines).map((line) => String(line)).join("\n");
  }
  if (Array.isArray(options.body)) {
    return options.body.map((line) => String(line)).join("\n");
  }
  if (options.body !== undefined && options.body !== null) {
    return String(options.body);
  }
  return "";
};

globalThis.__hostrun_gitCommitSubject = function (options) {
  const subject = options.subject ?? options.message ?? options.title;
  if (subject === undefined || subject === null || String(subject).trim().length === 0) {
    throw new Error("tools.git.commit requires subject or message");
  }
  return String(subject);
};

globalThis.__hostrun_validateGitCommitMessage = function (subject, body, options) {
  if (subject.includes("\n")) {
    throw new Error("Git commit subject must be one line. Use body or bodyLines for details.");
  }
  if (options.allowEscapedNewlines === true) {
    return;
  }
  if (subject.includes("\\n") || body.includes("\\n")) {
    throw new Error(
      "Git commit message contains literal \\\\n. Use bodyLines or a template literal for multiline text."
    );
  }
};

globalThis.__hostrun_gitCommitMessage = function (options) {
  const subject = globalThis.__hostrun_gitCommitSubject(options);
  const body = globalThis.__hostrun_gitCommitBody(options);
  globalThis.__hostrun_validateGitCommitMessage(subject, body, options);
  return body.length > 0 ? `${subject}\n\n${body}` : subject;
};

globalThis.__hostrun_gitCommitPaths = function (options) {
  return globalThis.__hostrun_values(options.paths ?? options.files ?? options.path ?? options.file)
    .map((file) => String(file));
};

globalThis.__hostrun_gitCommitPathExists = function (cwd, file) {
  if (String(file).startsWith("/") || cwd === undefined || cwd === null) {
    return globalThis.fs.exists(file);
  }
  return globalThis.fs.exists(globalThis.path.join(cwd, file));
};

globalThis.__hostrun_existingGitCommitPaths = function (options, paths) {
  const cwd = options.cwd ?? options.repo;
  return paths.filter((file) => globalThis.__hostrun_gitCommitPathExists(cwd, file));
};

globalThis.__hostrun_gitCwdArgs = function (options) {
  const cwd = options.cwd ?? options.repo;
  return cwd === undefined || cwd === null || cwd === false ? [] : ["-C", String(cwd)];
};

globalThis.git = {
  commit: function (options = {}) {
    const requestedPaths = globalThis.__hostrun_gitCommitPaths(options);
    const paths = globalThis.__hostrun_existingGitCommitPaths(options, requestedPaths);
    const includeStaged = options.includeStaged === true;

    if (requestedPaths.length > 0 && paths.length === 0 && !includeStaged) {
      throw new Error("tools.git.commit found no existing files to add or commit");
    }

    const cwdArgs = globalThis.__hostrun_gitCwdArgs(options);
    if (paths.length > 0) {
      globalThis.cli.git(...cwdArgs, "add", "--", ...paths).run();
    }

    const args = [...cwdArgs];
    args.push("commit", "--file", "-");
    globalThis.__hostrun_addOption(args, "--amend", options.amend);
    globalThis.__hostrun_addOption(args, "--no-edit", options.noEdit);
    globalThis.__hostrun_addOption(args, "--allow-empty", options.allowEmpty);
    globalThis.__hostrun_addOption(args, "--no-verify", options.noVerify);
    globalThis.__hostrun_addOption(args, "--signoff", options.signoff);
    globalThis.__hostrun_addOption(args, "--all", options.all);
    if (paths.length > 0 && !includeStaged) {
      args.push("--only");
    }
    if (paths.length > 0 && !includeStaged) {
      args.push("--", ...paths);
    }

    return globalThis.cli.git(...args)
      .stdin.text(globalThis.__hostrun_gitCommitMessage(options))
      .run();
  }
};

globalThis.tools.git = globalThis.git;

globalThis.github = {
  createPR: function (options = {}) {
    const args = ["pr", "create"];
    const body = globalThis.__hostrun_githubPrBody(options);
    globalThis.__hostrun_validateGithubPrBody(body, options);

    globalThis.__hostrun_addOption(args, "--repo", options.repo);
    globalThis.__hostrun_addOption(args, "--base", options.base);
    globalThis.__hostrun_addOption(args, "--head", options.head);
    globalThis.__hostrun_addOption(args, "--title", options.title);
    if (body !== undefined) {
      args.push("--body-file", "-");
    } else {
      globalThis.__hostrun_addOption(args, "--body", options.body);
    }
    globalThis.__hostrun_addOption(args, "--draft", options.draft);
    globalThis.__hostrun_addOption(args, "--fill", options.fill);
    globalThis.__hostrun_addOption(args, "--fill-first", options.fillFirst);
    globalThis.__hostrun_addOption(args, "--fill-verbose", options.fillVerbose);
    globalThis.__hostrun_addOption(args, "--web", options.web);
    globalThis.__hostrun_addOption(args, "--no-maintainer-edit", options.maintainerEdit === false);
    globalThis.__hostrun_addOption(args, "--milestone", options.milestone);
    globalThis.__hostrun_addRepeatedOption(args, "--label", options.labels ?? options.label);
    globalThis.__hostrun_addRepeatedOption(args, "--reviewer", options.reviewers ?? options.reviewer);
    globalThis.__hostrun_addRepeatedOption(args, "--assignee", options.assignees ?? options.assignee);
    globalThis.__hostrun_addRepeatedOption(args, "--project", options.projects ?? options.project);

    const command = globalThis.cli.gh(...args);
    return body === undefined ? command.run() : command.stdin.text(body).run();
  }
};

globalThis.tools.github = globalThis.github;

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

globalThis.__hostrun_commandStdout = function (result) {
  if (result === null || result === undefined) {
    return "";
  }
  const stdout = result.stdout ?? "";
  return Array.isArray(stdout) ? stdout.join("\n") : String(stdout);
};

globalThis.__hostrun_commandOutput = function (result, name) {
  if (result === null || result === undefined) {
    return "";
  }
  const output = result[name] ?? "";
  return Array.isArray(output) ? output.join("\n") : String(output);
};

globalThis.__hostrun_parseCommandOutput = function (result, name, parser) {
  const parsed = parser(globalThis.__hostrun_commandOutput(result, name));
  return { ...result, [name]: parsed };
};

globalThis.__hostrun_parseRgFiles = function (result) {
  return globalThis.__hostrun_commandStdout(result)
    .lines()
    .filter((line) => line.length > 0)
    .unique();
};

globalThis.__hostrun_parseRgMatches = function (result) {
  return globalThis.__hostrun_commandStdout(result)
    .jsonLines()
    .filter((event) => event.type === "match")
    .map((event) => {
      const data = event.data ?? {};
      return {
        path: data.path?.text ?? "",
        lineNumber: data.line_number ?? null,
        line: data.lines?.text ?? "",
        submatches: (data.submatches ?? []).map((submatch) => ({
          text: submatch.match?.text ?? "",
          start: submatch.start ?? null,
          end: submatch.end ?? null
        }))
      };
    });
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
    const result = globalThis.rg.search(pattern, paths, { ...options, filesWithMatches: true }).stdout.lines();
    return globalThis.__hostrun_parseRgFiles(result);
  },
  matches: function (pattern, paths = [], options = {}) {
    const result = globalThis.rg.search(pattern, paths, { ...options, json: true }).stdout.text();
    return globalThis.__hostrun_parseRgMatches(result);
  }
};

globalThis.sqlite = {
  query: function (database, sql, options = {}) {
    const args = [];
    if (options.json !== false) {
      args.push("-json");
    }
    if (options.header) {
      args.push("-header");
    }
    if (options.mode) {
      args.push("-" + String(options.mode));
    }
    args.push(String(database), String(sql));
    return globalThis.__hostrun_commandBuilder("sqlite3", args);
  }
};

globalThis.kubectl = {
  get: function (resource, options = {}) {
    const args = ["get", String(resource)];
    if (options.name) {
      args.push(String(options.name));
    }
    globalThis.__hostrun_addOption(args, "--namespace", options.namespace);
    globalThis.__hostrun_addOption(args, "--all-namespaces", options.allNamespaces);
    if (options.selector) {
      args.push("--selector", String(options.selector));
    }
    args.push("-o", String(options.output ?? "json"));
    return globalThis.__hostrun_commandBuilder("kubectl", args);
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
      return this.run().text ?? "";
    },
    json: function () {
      state.response = { type: "json" };
      return this.run().json ?? null;
    },
    bytes: function () {
      state.response = { type: "bytes" };
      return this.run().body ?? [];
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

globalThis.__hostrun_urlJoin = function (baseUrl, url) {
  const text = String(url);
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(text)) {
    return text;
  }
  const base = String(baseUrl ?? "");
  if (text.startsWith("/")) {
    return base.replace(/\/+$/, "") + text;
  }
  return base.replace(/\/+$/, "") + "/" + text;
};

globalThis.__hostrun_cookieEntries = function (headers) {
  const value = headers?.["set-cookie"];
  if (value === undefined || value === null) {
    return [];
  }
  return Array.isArray(value) ? value : [value];
};

globalThis.__hostrun_storeCookie = function (jar, setCookie) {
  const [pair] = String(setCookie).split(";");
  const separator = pair.indexOf("=");
  if (separator <= 0) {
    return;
  }
  const name = pair.slice(0, separator).trim();
  const value = pair.slice(separator + 1).trim();
  if (name.length === 0) {
    return;
  }
  jar[name] = value;
};

globalThis.__hostrun_cookieHeader = function (jar) {
  const pairs = Object.entries(jar).map(([name, value]) => `${name}=${value}`);
  return pairs.length === 0 ? undefined : pairs.join("; ");
};

globalThis.__hostrun_mergeHeaders = function (...headers) {
  return Object.assign({}, ...headers.filter(Boolean));
};

globalThis.__hostrun_httpSession = function (defaults = {}) {
  const jar = { ...(defaults.cookies ?? {}) };
  const baseUrl = defaults.baseUrl ?? defaults.baseURL ?? "";
  const session = {
    cookies: jar,
    run: function (method, url, options = {}) {
      const cookie = globalThis.__hostrun_cookieHeader(jar);
      const requestOptions = {
        ...defaults,
        ...options,
        headers: globalThis.__hostrun_mergeHeaders(defaults.headers, cookie ? { Cookie: cookie } : undefined, options.headers)
      };
      delete requestOptions.baseUrl;
      delete requestOptions.baseURL;
      delete requestOptions.cookies;
      const response = globalThis.http.request(method, globalThis.__hostrun_urlJoin(baseUrl, url), requestOptions).run();
      for (const setCookie of globalThis.__hostrun_cookieEntries(response.headers)) {
        globalThis.__hostrun_storeCookie(jar, setCookie);
      }
      return response;
    },
    request: function (method, url, options = {}) {
      return globalThis.__hostrun_httpSessionRequestBuilder(session, method, url, options);
    },
    get: function (url, options = {}) {
      return this.request("GET", url, options);
    },
    post: function (url, options = {}) {
      return this.request("POST", url, options);
    },
    put: function (url, options = {}) {
      return this.request("PUT", url, options);
    },
    patch: function (url, options = {}) {
      return this.request("PATCH", url, options);
    },
    delete: function (url, options = {}) {
      return this.request("DELETE", url, options);
    },
    head: function (url, options = {}) {
      return this.request("HEAD", url, options);
    }
  };
  return session;
};

globalThis.__hostrun_httpSessionRequestBuilder = function (session, method, url, options = {}) {
  const builder = {
    run: function () {
      return session.run(method, url, options);
    },
    text: function () {
      return session.run(method, url, { ...options, response: { type: "text" } }).text ?? "";
    },
    json: function () {
      return session.run(method, url, { ...options, response: { type: "json" } }).json ?? null;
    },
    bytes: function () {
      return session.run(method, url, { ...options, response: { type: "bytes" } }).body ?? [];
    },
    save: function (path) {
      return session.run(method, url, { ...options, response: { type: "file", path } });
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
  },
  session: function (defaults = {}) {
    return globalThis.__hostrun_httpSession(defaults);
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
