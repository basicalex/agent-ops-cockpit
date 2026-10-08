import { afterAll, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createTools } from "../src/tools";

const dir = await mkdtemp(join(tmpdir(), "aoc-live-notes-"));
afterAll(() => rm(dir, { recursive: true, force: true }));
const note = (file: string) => createTools({ notesFile: file }).find(t => t.name === "save_note")!;

test("save_note appends one JSON line to a private inbox and reports only the time", async () => {
  const file = join(dir, "state", "notes.jsonl");
  const first = await note(file).execute({ text: "Call Klara about Payload" });
  await note(file).execute({ text: "Second", workspace: "voyager" });
  const body = JSON.parse((first.content[0] as { text: string }).text);
  expect(body.saved).toBe(true);
  expect(Object.keys(body).sort()).toEqual(["at", "saved"]);
  const lines = (await readFile(file, "utf8")).trim().split("\n").map(l => JSON.parse(l));
  expect(lines.map(l => [l.text, l.workspace])).toEqual([["Call Klara about Payload", null], ["Second", "voyager"]]);
  expect((await stat(file)).mode & 0o777).toBe(0o600);
});

test("save_note rejects empty or oversized notes and a full inbox", async () => {
  const file = join(dir, "full.jsonl");
  expect((await note(file).execute({ text: " " })).isError).toBe(true);
  expect((await note(file).execute({ text: "x".repeat(2001) })).isError).toBe(true);
  await writeFile(file, "x".repeat(1_000_001));
  const full = await note(file).execute({ text: "one more" });
  expect(full.isError).toBe(true);
  expect((full.content[0] as { text: string }).text).toContain("Note inbox is full");
});
