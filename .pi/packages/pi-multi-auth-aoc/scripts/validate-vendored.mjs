import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const root = process.cwd();
const includeRoots = [join(root, "src")];
const entryFiles = [join(root, "index.ts")];

function collectTsFiles(dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const stats = statSync(fullPath);
    if (stats.isDirectory()) {
      results.push(...collectTsFiles(fullPath));
      continue;
    }
    if (fullPath.endsWith(".ts")) {
      results.push(fullPath);
    }
  }
  return results;
}

const files = [
  ...entryFiles,
  ...includeRoots.flatMap((dir) => collectTsFiles(dir)),
];

let hadError = false;
for (const file of files) {
  const source = readFileSync(file, "utf8");
  const result = ts.transpileModule(source, {
    fileName: file,
    reportDiagnostics: true,
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.NodeNext,
      moduleResolution: ts.ModuleResolutionKind.NodeNext,
      allowImportingTsExtensions: false,
      verbatimModuleSyntax: false,
    },
  });

  for (const diagnostic of result.diagnostics ?? []) {
    if (diagnostic.category !== ts.DiagnosticCategory.Error) {
      continue;
    }
    hadError = true;
    const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
    const relativePath = relative(root, file);
    if (diagnostic.file && typeof diagnostic.start === "number") {
      const position = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
      console.error(`${relativePath}:${position.line + 1}:${position.character + 1} ${message}`);
    } else {
      console.error(`${relativePath}: ${message}`);
    }
  }
}

if (hadError) {
  process.exit(1);
}

console.log(`Validated ${files.length} TypeScript file(s) in vendored pi-multi-auth-aoc.`);
