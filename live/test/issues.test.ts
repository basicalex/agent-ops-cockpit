import { afterEach, expect, test } from "bun:test";
import { resolve } from "node:path";

import { setRunner } from "../src/exec";
import { getIssueState, listIssues } from "../src/issues";
afterEach(() => setRunner(null));

function fixtures() {
  setRunner(async argv => {
    const filename = argv[0] !== "gh" ? "state.json" : argv[2] === "list" ? "list.json" : "comments.json";
    return { stdout: await Bun.file(resolve(import.meta.dir, "fixtures/w2", filename)).text(), stderr: "", code: 0, truncated: false };
  });
}

test("canonical snapshot replaces journal re-parsing and excludes marked comments", async () => {
  fixtures();
  const state = await getIssueState("/synthetic/repo", 10);
  expect(state.schema).toBe("aoc.issue.state/v1");
  expect(state.issue).toBe(10);
  expect(state.state).toBe("running");
  expect(state.objective).toBe("Read context. api_key=[REDACTED]");
  expect(state.blockers).toEqual(["password=[REDACTED]"]);
  expect(state.events[1]?.change).toBe("token=[REDACTED]");
  expect(state.events[1]?.evidence).toBe("Bearer [REDACTED]");
  expect(state.events[1]?.blockers).toBe("client_secret=[REDACTED]");
  expect(state.commentTotal).toBe(6);
  expect(state.recentComments.map(comment => comment.author)).toEqual(["", "bob", "carol"]);
  expect(state.recentComments[0]?.body).toBe("Second plain comment [REDACTED]");
  expect(state.recentComments[2]?.body).toContain("<!-- aoc-journal");
  expect(state).not.toHaveProperty("latestJournal");
  expect(state).not.toHaveProperty("linkedPrs");
  expect(state).not.toHaveProperty("body");
});

test("comment and event limits select newest entries and zero means none", async () => {
  fixtures();
  const state = await getIssueState("/synthetic/repo", 10, { comments: 1, events: 1 });
  expect(state.events.map(event => event.kind)).toEqual(["validation"]);
  expect(state.recentComments.map(comment => comment.author)).toEqual(["carol"]);
  const empty = await getIssueState("/synthetic/repo", 10, { comments: 0, events: 0 });
  expect(empty.recentComments).toEqual([]);
  expect(empty.events).toEqual([]);
  expect(empty.commentTotal).toBe(6);
});

test("comment redaction happens before the body cap", async () => {
  setRunner(async argv => ({ stdout: argv[0] !== "gh" ? await Bun.file(resolve(import.meta.dir, "fixtures/w2/state.json")).text() : JSON.stringify({ comments: [{ author: { login: "alice" }, createdAt: "2026-10-03T00:00:00Z", url: "https://example.com", body: "x".repeat(1490) + " password=synthetic-comment-password" }] }), stderr: "", code: 0, truncated: false }));
  const state = await getIssueState("/synthetic/repo", 10);
  expect(state.recentComments[0]?.body.length).toBe(1500);
  expect(state.recentComments[0]?.body).not.toContain("synthetic");
});

test("issue listing applies filters and redacts text", async () => {
  fixtures();
  const rows = await listIssues("/synthetic/repo", { state: "all", label: "agent-running", limit: 1 });
  expect(rows).toEqual([{ number: 10, title: "Read-only [REDACTED]", state: "OPEN", labels: ["agent-running"], updatedAt: "2026-10-03T10:00:00Z", url: "https://github.com/example/repo/issues/10" }]);
});

test("failed or truncated reads never fabricate issue state", async () => {
  setRunner(async () => ({ stdout: "", stderr: "password=synthetic-error-password", code: 1, truncated: false }));
  await expect(getIssueState("/synthetic/repo", 10)).rejects.toThrow("password=[REDACTED]");
  setRunner(async () => ({ stdout: "{}", stderr: "", code: 0, truncated: true }));
  await expect(listIssues("/synthetic/repo")).rejects.toThrow("output limit");
});
