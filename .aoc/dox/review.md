# AOC DOX Review Packet

## Summary
- Schema: `aoc.dox.v1`
- Directories scanned: 507
- Budget status: `Ok`
- Proposed local AGENTS.md files: 17
- Rejected candidates listed: 490
- Persisted routes: 0
- Candidate-derived routes: 17

## Proposed AGENTS.md routes
| Target AGENTS.md | Scope | Purpose | Decision | Score | Bytes |
|---|---|---|---|---:|---:|
| .omp/extensions/AGENTS.md | .omp/extensions | critic-approved compressed create: `.omp/extensions` is root-only/insufficient in `.aoc/dox/map.json`; local rules govern host-facing Pi extension command/tool surfaces. | Create | 9 | 2336 |
| bin/AGENTS.md | bin | critic-approved compressed create: `bin` is root-only/insufficient in `.aoc/dox/map.json`; local rules protect public PATH entrypoints and generated/cache boundaries not covered by | Create | 9 | 1864 |
| crates/aoc-agent-wrap-rs/src/AGENTS.md | crates/aoc-agent-wrap-rs/src | critic-approved compressed create: root AGENTS does not cover Pulse wire shape, secret-safe child env, redaction, stop escalation, or detached Insight state. | Create | 8 | 1464 |
| crates/aoc-cli/AGENTS.md | crates/aoc-cli | critic-approved compressed create: package owns the user-facing `aoc` binary surface plus stateful Taskmaster, DOX, and map writes. | Create | 8 | 792 |
| crates/aoc-core/src/AGENTS.md | crates/aoc-core/src | critic-approved compressed create: subtree exports shared serde/wire/storage contracts where compatibility, framing, redaction, and budget constants are durable local invariants. | Create | 8 | 1203 |
| crates/aoc-hub-rs/src/AGENTS.md | crates/aoc-hub-rs/src | critic-approved compressed create: root rules do not cover hub protocol/session/UDS transport invariants. | Create | 8 | 1049 |
| crates/aoc-installer/src/AGENTS.md | crates/aoc-installer/src | critic-approved compressed create: root rules do not protect live installer side effects, downloader/source validation, or host PATH process spawning. | Create | 8 | 940 |
| crates/aoc-mind/src/AGENTS.md | crates/aoc-mind/src | critic-approved compressed create: root rules do not cover Mind state layout, legacy compatibility, file-lock/store-lease coordination, deterministic fallback, manifests, watermark | Create | 8 | 2103 |
| crates/aoc-mind/src/bin/AGENTS.md | crates/aoc-mind/src/bin | critic-approved compressed create: binary-specific machine output, exit-code, long-running loop, finalization write-order, and project-scoped external memory checks remain additive | Create | 8 | 1595 |
| crates/aoc-mission-control/src/AGENTS.md | crates/aoc-mission-control/src | critic-approved compressed create: Mission Control has Pulse sequencing/session filters, config/env parsing, offline fallback rendering, Zellij vs standalone launch paths, and Mind | Create | 8 | 1015 |
| crates/aoc-opencode-adapter/src/AGENTS.md | crates/aoc-opencode-adapter/src | critic-approved compressed create: append-only NDJSON ingestion, redaction, deterministic identity, lineage compatibility, and restart attribution are adapter-specific invariants a | Create | 8 | 1081 |
| crates/aoc-pi-adapter/src/AGENTS.md | crates/aoc-pi-adapter/src | critic-approved compressed create: Pi session header identity, cursor semantics, redaction, source attrs/lineage, and compaction rebuildability are durable adapter invariants. | Create | 8 | 1137 |
| crates/aoc-segment-routing/src/AGENTS.md | crates/aoc-segment-routing/src | critic-approved compressed create: routing precedence/provenance, uncertainty fallback, and manual override audit semantics are compact and operational. | Create | 8 | 884 |
| crates/aoc-storage/src/AGENTS.md | crates/aoc-storage/src | critic-approved compressed create: storage schema/versioning, secret rejection, lease ownership, segment-route replacement, and compaction round trips are durable SQLite boundary i | Create | 8 | 1267 |
| crates/aoc-task-attribution/src/AGENTS.md | crates/aoc-task-attribution/src | critic-approved compressed create: root AGENTS does not cover Mind artifact-task confidence, provenance, dedup, or extraction boundaries. | Create | 8 | 1163 |
| crates/aoc-taskmaster/src/AGENTS.md | crates/aoc-taskmaster/src | critic-approved compressed create: root AGENTS does not cover the TUI writer path, root resolution, terminal restoration, and watcher bounds. | Create | 8 | 1580 |
| crates/aoc-yazi-mermaid/src/AGENTS.md | crates/aoc-yazi-mermaid/src | critic-approved compressed create: root AGENTS has no Yazi preview CLI, stdout, cache identity, atomic render, or Markdown fence contract. | Create | 7 | 1336 |

## Rejected routes
| Path | Score | Reason |
|---|---:|---|
| . | 5 | below min_score |
| .agents | 0 | below min_score |
| .agents/skills | 0 | below min_score |
| .aoc | 1 | below min_score |
| .aoc/insight | 1 | below min_score |
| .aoc/insight/sessions | 1 | below min_score |
| .aoc/layouts | 1 | below min_score |
| .aoc/logs | 1 | below min_score |
| .aoc/map | 1 | below min_score |
| .aoc/map/assets | 1 | below min_score |
| .aoc/map/diagrams | 1 | below min_score |
| .aoc/map/pages | 1 | below min_score |
| .aoc/mind | 1 | below min_score |
| .aoc/mind/insight | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T045835Z_b7f28146170d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T045853Z_60ce22c47989 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T045913Z_8b882fc6a9e2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T045928Z_449125ade652 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T045931Z_4b14909406f0 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T051259Z_56e5d0b19991 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T052758Z_4d69d8210e07 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T054755Z_5942d6a0c3e9 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T062154Z_e567ee3b4a75 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T063718Z_128bb334e051 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T070845Z_8de503c0de7b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T075624Z_efdf3a87fe64 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T080520Z_01bfaee0f4d2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T082539Z_f38cd56fae19 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T145423Z_238bf64b33e8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T150143Z_f61fbf2df0a3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T150225Z_067b62c1be0c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T150537Z_68fcfdcf9574 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260224T150810Z_7316e5b73a3f | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T050626Z_7d7411188dd6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T063536Z_455e723b6153 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T142943Z_071f2c3ca4b5 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T144418Z_81083ec7d58c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T144804Z_a02ebb039a3a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T151844Z_10ba54fd1a92 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T152335Z_2edf1ed0c87d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260225T153751Z_1441ae59128b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260226T061219Z_bd366d84b9dd | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260226T064702Z_1b62f8a710ed | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260226T085226Z_71ed284e53fa | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260226T152602Z_0f9e45a6e200 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T053100Z_148459e24a53 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T064151Z_e04397c7c36e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T082152Z_ad05c07d21c2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T124007Z_96d4cfa70774 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T134850Z_bf214ea9292d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T134912Z_f99c63f60f20 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260227T134935Z_4a930d5a000d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260228T130921Z_db3fd41a9d00 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260228T204451Z_47ca7a526842 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260301T073833Z_056a7f1c5b6e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260301T091124Z_1a1e13aa5bf3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260302T063435Z_f5372ed55a7b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260304T182416Z_8d181b49c7c3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260308T184746Z_b7857eea4f2a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260309T100901Z_05c0f7715389 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260310T193948Z_2ec95cb15f65 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260313T092918Z_3df2dff7ff0c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260315T202951Z_d288fd8528a8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260317T141455Z_fbfe4c2080ba | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260318T093822Z_80a90f54c38c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260318T183714Z_124d56855c31 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260318T202650Z_325689923059 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260320T111830Z_ea3775a9091c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260320T115114Z_2ec6c99b89aa | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260320T120244Z_4c64bdfe7c4d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260321T094331Z_7554e0659817 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260321T101036Z_aab8a25d0aa7 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260321T103111Z_c730cb41ced3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260321T110042Z_e89c7efcc9c3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260323T145218Z_00edfec693c7 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260323T145412Z_a75652ea9306 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260323T170708Z_921364fbccdd | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260324T173423Z_46848af887bd | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260324T184856Z_378dfdd0d7e4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260326T102140Z_101c3a1634eb | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260326T120419Z_3cc2d04d28a1 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260327T110601Z_c58e2b0425b5 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260329T162732Z_0bfd578d86a3 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260402T073226Z_599fd332ab9d | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260403T073905Z_b65bc399d369 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260403T075416Z_28a57f61a679 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260405T153342Z_71203b3b3f6c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260405T153629Z_586ac3a4e6fd | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260406T124916Z_1288bca96a2b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260406T125012Z_8fcf4f602273 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260406T125748Z_aa350434fc8a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260406T185643Z_2715a8e83269 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260407T193538Z_e3d5f8a5753b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260409T142140Z_c6d3835a440c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260409T174204Z_85c371e27ba6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260409T195016Z_7b25f5bdcd32 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260410T105843Z_52fbcb33010e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260411T093949Z_e0a9a59715b8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260411T123053Z_7fdb8cd88e84 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260412T194927Z_aecc979fb3f7 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260412T195807Z_d6f6504db861 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260412T203643Z_a945b261be9f | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260412T204935Z_e2c8d7778e14 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260412T205005Z_7bd7bf2b99e4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T085416Z_1622b88e52e6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T085454Z_f6486e482e15 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T090503Z_f0a346ba2e80 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T095709Z_3225139e2ef6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T210019Z_2d418ec6f2c0 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T212722Z_79ca1341dce2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260413T212823Z_6f9b82d8f99e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260416T141852Z_8af1bebcf84b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260416T141852Z_9d19669a104f | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260418T100103Z_197e6fd66a7e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260418T100103Z_da43bc8800b6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260420T143724Z_55724c7c03a4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260420T143737Z_a8fc7397f7d0 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260420T143743Z_3733d4f34af5 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260420T143930Z_722c15d649a9 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T130335Z_2c77995c34c8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T130335Z_f5a127bc3c49 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T131649Z_009dfc33b996 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T131751Z_1ce094b605a4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T131751Z_fd5afec461d6 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135032Z_479ea005bbfd | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135032Z_e47e1fe6c06c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135637Z_0bf3ebd00d64 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135637Z_3e875fc28252 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135655Z_1d012f47927a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135655Z_bdd064ab509b | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135940Z_5158f91a1c30 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260421T135940Z_e329da1eebc9 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T134853Z_776802b0cd67 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T135536Z_f64494c2fafe | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T143058Z_676b09da0e1f | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T143058Z_69a6f1a8e7bb | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T150414Z_1208599e9ebf | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T150414Z_c4e5c4190015 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T150603Z_8dbbf70b9701 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T150603Z_ee9dc72eccd1 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T150826Z_0c7b34127e28 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T151457Z_7cec8fa29463 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T151457Z_d0a4c57685f1 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T170932Z_2ddb5eab7820 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T170932Z_bdf7c370d1ce | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T171011Z_2fc05f1e41f8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T171011Z_a77c82b13360 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T210212Z_0aed8e76217c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T210212Z_c8ff889b511e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T210757Z_331d9586a15c | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T210757Z_bc84574c8b79 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T212122Z_cdf7fdb8b512 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260422T212122Z_da1211fb17ac | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T083709Z_308e8a7e0539 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T155449Z_d061b3df057f | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T160203Z_97e37267411e | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T160203Z_fbb87bfcccf9 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T174441Z_bc67ee968790 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260424T174441Z_fccc48fbc850 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260425T143937Z_36539c60a7da | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260425T143937Z_9c1e4a350e03 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260427T090812Z_2fdb47c31bf5 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260427T094525Z_3f70c8eac8f1 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260427T095817Z_465c230c47e4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260427T095817Z_511fc1d8d102 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260427T101231Z_76cfb13a3056 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260428T163322Z_5755e8d37ae4 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260428T175556Z_9413ad522016 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260429T084818Z_15b5abf27c85 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260506T144654Z_2e3590d6d27a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260506T152221Z_a78ed21c7a41 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260507T081353Z_03c058647294 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260507T081353Z_961a8aab35ff | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260507T094340Z_0cfa7481ba8a | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260507T101331Z_1e9c607fe7f8 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260513T194136Z_7ef3899158c7 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260513T194451Z_272085e1d942 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260513T194451Z_3d2217e18cd7 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260515T081302Z_e88f68a73593 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260515T081302Z_fecf1318dfe2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260515T160629Z_ac971a1078c2 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260515T164541Z_009bb2538d05 | 1 | below min_score |
| .aoc/mind/insight/mind-backfill_20260515T164541Z_0802bd8971ce | 1 | below min_score |
| .aoc/open-design | 1 | below min_score |
| .aoc/presets | 1 | below min_score |
| .aoc/presets/.aoc-backups | 1 | below min_score |
| .aoc/presets/.aoc-backups/design.aoc-dirty-backup.20260517T125757Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/design.aoc-dirty-backup.20260517T125757Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/design.aoc-unmanaged-backup.20260428T172357Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/design.aoc-unmanaged-backup.20260428T172357Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/hyperframes.aoc-dirty-backup.20260517T125757Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/hyperframes.aoc-dirty-backup.20260517T125757Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/hyperframes.aoc-unmanaged-backup.20260428T172357Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/hyperframes.aoc-unmanaged-backup.20260428T172357Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/ops.aoc-unmanaged-backup.20260517T125757Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/ops.aoc-unmanaged-backup.20260517T125757Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/research.aoc-unmanaged-backup.20260517T125757Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/research.aoc-unmanaged-backup.20260517T125757Z/components | 1 | below min_score |
| .aoc/presets/.aoc-backups/test.aoc-unmanaged-backup.20260523T140701Z | 1 | below min_score |
| .aoc/presets/.aoc-backups/test.aoc-unmanaged-backup.20260523T140701Z/components | 1 | below min_score |
| .aoc/presets/design | 1 | below min_score |
| .aoc/presets/design/components | 1 | below min_score |
| .aoc/presets/hyperframes | 1 | below min_score |
| .aoc/presets/hyperframes/components | 1 | below min_score |
| .aoc/presets/ops | 1 | below min_score |
| .aoc/presets/ops/components | 1 | below min_score |
| .aoc/presets/research | 1 | below min_score |
| .aoc/presets/research/components | 1 | below min_score |
| .aoc/presets/test | 1 | below min_score |
| .aoc/presets/test/components | 1 | below min_score |
| .aoc/prompts | 3 | below min_score |
| .aoc/prompts-optional | 3 | below min_score |
| .aoc/prompts-optional/pi | 3 | below min_score |
| .aoc/prompts/pi | 3 | below min_score |
| .aoc/services | 1 | below min_score |
| .aoc/services/searxng | 1 | below min_score |
| .aoc/skills | 1 | below min_score |
| .aoc/skills-optional | 1 | below min_score |
| .aoc/skills-optional/aoc-hyperframes | 1 | below min_score |
| .aoc/skills-optional/aoc-hyperframes/playbooks | 1 | below min_score |
| .aoc/skills-optional/aoc-hyperframes/templates | 1 | below min_score |
| .aoc/skills-optional/gsap | 1 | below min_score |
| .aoc/skills-optional/hyperframes | 1 | below min_score |
| .aoc/skills-optional/hyperframes-cli | 1 | below min_score |
| .aoc/skills-optional/website-to-hyperframes | 1 | below min_score |
| .aoc/skills/aoc-init-ops | 1 | below min_score |
| .aoc/skills/prd-rpg-authoring | 1 | below min_score |
| .aoc/skills/rlm-analysis | 1 | below min_score |
| .aoc/skills/teach-workflow | 1 | below min_score |
| .aoc/skills/tm-cc | 1 | below min_score |
| .aoc/skills/zellij-theme-ops | 1 | below min_score |
| .aoc/stm | 1 | below min_score |
| .aoc/stm/archive | 1 | below min_score |
| .aoc/tools | 1 | below min_score |
| .aoc/tools/obscura | 1 | below min_score |
| .github | -2 | below min_score |
| .github/ISSUE_TEMPLATE | -2 | below min_score |
| .github/workflows | -2 | below min_score |
| .omp | -2 | below min_score |
| .omp/agents | 3 | below min_score |
| .omp/skills | 1 | below min_score |
| .omp/skills/aoc-dox-cartography | 1 | below min_score |
| .pi | 1 | below min_score |
| .pi/agents | 3 | below min_score |
| .pi/extensions | 3 | below min_score |
| .pi/extensions/.aoc-backups | 3 | below min_score |
| .pi/extensions/.aoc-backups/aoc-presets.aoc-dirty-backup.20260517T125753Z | 3 | below min_score |
| .pi/extensions/.aoc-backups/aoc-presets.aoc-unmanaged-backup.20260428T172352Z | 3 | below min_score |
| .pi/extensions/.aoc-backups/subagent.aoc-unmanaged-backup.20260518T141418Z | 3 | below min_score |
| .pi/extensions/aoc-presets | 3 | below min_score |
| .pi/extensions/lib | 3 | below min_score |
| .pi/extensions/subagent | 3 | below min_score |
| .pi/packages | 1 | below min_score |
| .pi/packages/.aoc-backups | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T172352Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260428T173228Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260429T084747Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260513T151222Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260523T140653Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260524T085159Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260530T135904Z/src/usage | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/debug | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/scripts | 1 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/src | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/src/balancer | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/src/formatters | 3 | below min_score |
| .pi/packages/.aoc-backups/pi-multi-auth-aoc.aoc-dirty-backup.20260531T184127Z/src/usage | 3 | below min_score |
| .pi/packages/pi-multi-auth-aoc | 1 | below min_score |
| .pi/packages/pi-multi-auth-aoc/scripts | 1 | below min_score |
| .pi/packages/pi-multi-auth-aoc/src | 3 | below min_score |
| .pi/packages/pi-multi-auth-aoc/src/balancer | 3 | below min_score |
| .pi/packages/pi-multi-auth-aoc/src/formatters | 3 | below min_score |
| .pi/packages/pi-multi-auth-aoc/src/usage | 3 | below min_score |
| .pi/prompts | 3 | below min_score |
| .pi/prompts-optional | 3 | below min_score |
| .pi/prompts-optional/production-hidden | 3 | below min_score |
| .pi/prompts/.aoc-backups | 3 | below min_score |
| .pi/skills | 1 | below min_score |
| .pi/skills/.aoc-backups | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-commit.aoc-unmanaged-backup.20260428T172353Z | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-dirty-backup.20260513T151225Z | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-dirty-backup.20260513T151225Z/playbooks | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-dirty-backup.20260513T151225Z/templates | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-unmanaged-backup.20260428T172353Z | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-unmanaged-backup.20260428T172353Z/playbooks | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-hyperframes.aoc-unmanaged-backup.20260428T172353Z/templates | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-init-ops.aoc-dirty-backup.20260610T164412Z | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-init-ops.aoc-unmanaged-backup.20260428T172353Z | 1 | below min_score |
| .pi/skills/.aoc-backups/aoc-map.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/architecture-design.aoc-unmanaged-backup.20260517T075407Z | 1 | below min_score |
| .pi/skills/.aoc-backups/custom-layout-ops.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-diff.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-director.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-handoff.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-premium-ui.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-redesign.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-review.aoc-unmanaged-backup.20260428T172354Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-spec.aoc-unmanaged-backup.20260428T172355Z | 1 | below min_score |
| .pi/skills/.aoc-backups/design-tokens.aoc-unmanaged-backup.20260428T172355Z | 1 | below min_score |
| .pi/skills/.aoc-backups/enforce-dashboard-ux-guardrails.aoc-unmanaged-backup.20260523T140658Z | 1 | below min_score |
| .pi/skills/.aoc-backups/frontend-design.aoc-unmanaged-backup.20260513T151231Z | 1 | below min_score |
| .pi/skills/.aoc-backups/funnel-design.aoc-dirty-backup.20260530T135918Z | 1 | below min_score |
| .pi/skills/.aoc-backups/funnel-design.aoc-dirty-backup.20260530T135918Z/references | 1 | below min_score |
| .pi/skills/.aoc-backups/funnel-design.aoc-unmanaged-backup.20260513T151232Z | 1 | below min_score |
| .pi/skills/.aoc-backups/funnel-design.aoc-unmanaged-backup.20260513T151232Z/references | 1 | below min_score |
| .pi/skills/.aoc-backups/motion-director.aoc-unmanaged-backup.20260428T172355Z | 1 | below min_score |
| .pi/skills/.aoc-backups/prd-rpg-authoring.aoc-unmanaged-backup.20260428T172355Z | 1 | below min_score |
| .pi/skills/.aoc-backups/rlm-analysis.aoc-unmanaged-backup.20260428T172356Z | 1 | below min_score |
| .pi/skills/.aoc-backups/safe-gamification.aoc-unmanaged-backup.20260517T075409Z | 1 | below min_score |
| .pi/skills/.aoc-backups/spec-rpg-authoring.aoc-unmanaged-backup.20260429T084758Z | 1 | below min_score |
| .pi/skills/.aoc-backups/teach-workflow.aoc-dirty-backup.20260609T203116Z | 1 | below min_score |
| .pi/skills/.aoc-backups/teach-workflow.aoc-unmanaged-backup.20260428T172356Z | 1 | below min_score |
| .pi/skills/.aoc-backups/tm-cc.aoc-unmanaged-backup.20260428T172356Z | 1 | below min_score |
| .pi/skills/.aoc-backups/vercel-cli.aoc-unmanaged-backup.20260428T172356Z | 1 | below min_score |
| .pi/skills/.aoc-backups/web-research.aoc-unmanaged-backup.20260428T172357Z | 1 | below min_score |
| .pi/skills/.aoc-backups/zellij-theme-ops.aoc-unmanaged-backup.20260428T172357Z | 1 | below min_score |
| .pi/skills/agent-browser | 1 | below min_score |
| .pi/skills/animejs-core-api | 1 | below min_score |
| .pi/skills/animejs-core-api/references | 1 | below min_score |
| .pi/skills/animejs-performance-a11y | 1 | below min_score |
| .pi/skills/animejs-react-integration | 1 | below min_score |
| .pi/skills/animejs-reviewer | 1 | below min_score |
| .pi/skills/animejs-scene-planner | 1 | below min_score |
| .pi/skills/animejs-scroll-interaction | 1 | below min_score |
| .pi/skills/animejs-svg-motion | 1 | below min_score |
| .pi/skills/animejs-text-splitting | 1 | below min_score |
| .pi/skills/animejs-timelines | 1 | below min_score |
| .pi/skills/aoc-gaps | 1 | below min_score |
| .pi/skills/aoc-hyperframes | 1 | below min_score |
| .pi/skills/aoc-hyperframes/playbooks | 1 | below min_score |
| .pi/skills/aoc-hyperframes/templates | 1 | below min_score |
| .pi/skills/aoc-init-ops | 1 | below min_score |
| .pi/skills/aoc-map | 1 | below min_score |
| .pi/skills/aoc-understand | 1 | below min_score |
| .pi/skills/architecture-design | 1 | below min_score |
| .pi/skills/custom-layout-ops | 1 | below min_score |
| .pi/skills/design-diff | 1 | below min_score |
| .pi/skills/design-director | 1 | below min_score |
| .pi/skills/design-handoff | 1 | below min_score |
| .pi/skills/design-premium-ui | 1 | below min_score |
| .pi/skills/design-redesign | 1 | below min_score |
| .pi/skills/design-review | 1 | below min_score |
| .pi/skills/design-spec | 1 | below min_score |
| .pi/skills/design-tokens | 1 | below min_score |
| .pi/skills/enforce-dashboard-ux-guardrails | 1 | below min_score |
| .pi/skills/frontend-design | 1 | below min_score |
| .pi/skills/funnel-design | 1 | below min_score |
| .pi/skills/funnel-design/references | 1 | below min_score |
| .pi/skills/gsap | 1 | below min_score |
| .pi/skills/hyperframes | 1 | below min_score |
| .pi/skills/hyperframes-cli | 1 | below min_score |
| .pi/skills/motion-director | 1 | below min_score |
| .pi/skills/omarchy-theme-ops | 1 | below min_score |
| .pi/skills/prd-rpg-authoring | 1 | below min_score |
| .pi/skills/rlm-analysis | 1 | below min_score |
| .pi/skills/safe-gamification | 1 | below min_score |
| .pi/skills/spec-rpg-authoring | 1 | below min_score |
| .pi/skills/teach-workflow | 1 | below min_score |
| .pi/skills/tm-cc | 1 | below min_score |
| .pi/skills/tmcc | 1 | below min_score |
| .pi/skills/vercel-cli | 1 | below min_score |
| .pi/skills/web-research | 1 | below min_score |
| .pi/skills/website-to-hyperframes | 1 | below min_score |
| .pi/skills/zellij-theme-ops | 1 | below min_score |
| .pi/tmp | 1 | below min_score |
| .pi/tmp/subagents | 3 | below min_score |
| .pi/tmp/subagents/sj_mp5rj3ng_explorer-agent_0ynb8j9y | 3 | below min_score |
| .pi/tmp/subagents/sj_mp5sn6v0_specialist-core_525tuch0 | 3 | below min_score |
| .pi/tmp/subagents/sj_mp5sn9c2_planner-agent_gx9edlsi | 3 | below min_score |
| .pi/tmp/subagents/sj_mp5t4wxn_planner-agent_5qxp9fah | 3 | below min_score |
| .pi/tmp/subagents/sj_mp5t72v3_planner-agent_1dcrt7b4 | 3 | below min_score |
| .pi/tmp/subagents/sj_mp6lxbyx_planner-agent_sk40illa | 3 | below min_score |
| .pi/tmp/subagents/sj_mp6m84nz_planner-agent_resr5g72 | 3 | below min_score |
| .pi/tmp/subagents/sj_mp6me1ar_planner-agent_djw530d6 | 3 | below min_score |
| .pi/tmp/subagents/sj_mp6ntbtw_planner-agent_a1csku6x | 3 | below min_score |
| .pi/tmp/subagents/sj_mp6o6204_planner-agent_pk8gvndf | 3 | below min_score |
| .pi/tmp/subagents/sj_mp72g5tj_planner-agent_g6bhi7tm | 3 | below min_score |
| .pi/tmp/subagents/sj_mp740523_planner-agent_2ni71d0u | 3 | below min_score |
| .pi/tmp/subagents/sj_mpf94y1e_explorer-agent_5mut0t4s | 3 | below min_score |
| .pi/tmp/subagents/sj_mph75433_code-review-agent_atrw4but | 3 | below min_score |
| .taskmaster | 1 | below min_score |
| .taskmaster/docs | 1 | below min_score |
| .taskmaster/docs/prds | 1 | below min_score |
| .taskmaster/docs/specs | 1 | below min_score |
| .taskmaster/reports | 1 | below min_score |
| .taskmaster/tasks | 1 | below min_score |
| .taskmaster/templates | 1 | below min_score |
| bin/__pycache__ | 7 | missing evidence or verification |
| config | -2 | below min_score |
| crates | 1 | below min_score |
| crates/.aoc | 4 | below min_score |
| crates/.aoc/logs | 4 | below min_score |
| crates/.taskmaster | 4 | below min_score |
| crates/aoc-agent-wrap-rs | 4 | below min_score |
| crates/aoc-cli/assets | 4 | below min_score |
| crates/aoc-cli/src | 7 | missing evidence or verification |
| crates/aoc-control | 4 | below min_score |
| crates/aoc-control/src | 7 | missing evidence or verification |
| crates/aoc-core | 4 | below min_score |
| crates/aoc-hub-rs | 4 | below min_score |
| crates/aoc-installer | 4 | below min_score |
| crates/aoc-mind | 4 | below min_score |
| crates/aoc-mind/tests | 4 | below min_score |
| crates/aoc-mission-control | 4 | below min_score |
| crates/aoc-opencode-adapter | 4 | below min_score |
| crates/aoc-pi-adapter | 4 | below min_score |
| crates/aoc-segment-routing | 4 | below min_score |
| crates/aoc-storage | 4 | below min_score |
| crates/aoc-storage/migrations | 4 | below min_score |
| crates/aoc-task-attribution | 4 | below min_score |
| crates/aoc-taskmaster | 4 | below min_score |
| crates/aoc-yazi-mermaid | 4 | below min_score |
| docs | -2 | below min_score |
| docs/archive | -2 | below min_score |
| docs/archive/research | -2 | below min_score |
| docs/assets | -2 | below min_score |
| docs/maintainer | -2 | below min_score |
| docs/operator | -2 | below min_score |
| docs/reference | -2 | below min_score |
| docs/security | -2 | below min_score |
| herdr | -2 | below min_score |
| install | -2 | below min_score |
| legacy | -2 | below min_score |
| legacy/opencode | -2 | below min_score |
| legacy/opencode/config | -2 | below min_score |
| legacy/opencode/scripts | -2 | below min_score |
| lib | -2 | below min_score |
| lib/aoc_cleanup | -2 | below min_score |
| lib/aoc_cleanup/__pycache__ | -2 | below min_score |
| micro | -2 | below min_score |
| scripts | 5 | below min_score |
| scripts/pi | 5 | below min_score |
| scripts/pi/__pycache__ | 5 | below min_score |
| shellcheck-v0.10.0 | -2 | below min_score |
| vendor | -2 | below min_score |
| yazi | -2 | below min_score |
| yazi/plugins | -2 | below min_score |
| yazi/plugins/aoc-help-exit.yazi | -2 | below min_score |
| yazi/plugins/aoc-help-toggle.yazi | -2 | below min_score |
| yazi/plugins/aoc-mermaid-open.yazi | -2 | below min_score |
| yazi/plugins/aoc-mermaid.yazi | -2 | below min_score |
| yazi/plugins/aoc-open-explorer.yazi | -2 | below min_score |
| yazi/plugins/aoc-open.yazi | -2 | below min_score |
| yazi/plugins/aoc-pane-toggle.yazi | -2 | below min_score |
| yazi/plugins/aoc-title.yazi | -2 | below min_score |
| zellij | -2 | below min_score |
| zellij/layouts | -2 | below min_score |

## Rendered local contracts
### `.omp/extensions/AGENTS.md`
Scope: `.omp/extensions`
Purpose: critic-approved compressed create: `.omp/extensions` is root-only/insufficient in `.aoc/dox/map.json`; local rules govern host-facing Pi extension command/tool surfaces.

```md
# Repository Guidelines

Scope: `.omp/extensions`

## Local Contracts
- Expose Pi capabilities only through `ExtensionAPI.registerTool`/`registerCommand`; keep tool parameters typed with TypeBox/StringEnum schemas and keep slash-command arguments routed through explicit modes, aliases, and completions.
- For subprocess-backed tools, scope cwd under the project root, bound/truncate output, enforce timeout and AbortSignal cleanup, use `spawn(..., { shell:false })`, and surface nonzero/timeout results as unavailable evidence rather than success.
- Slash-command extensions should hand workflow prompts to the agent with `pi.sendMessage({ customType, display:true, content, details }, { triggerTurn:true })`; include cwd/scope in details and use `ctx.ui.notify` only as fallback.
- Tool descriptions and `promptGuidelines` must encode operational limits and write-safety for each exposed capability; write/apply/install/init/sync-style actions require a safe schema mode and matching prompt guidance before exposure.

## Verification
- `bun --check .omp/extensions/<changed-extension>.ts`

## Do Not
- Do not add ad-hoc string-command parsing, untyped parameter bags, or hidden host actions outside the registered Pi extension API.
- Do not expose apply/write/install/init/index/sync actions by name or implication unless the safe mode is encoded in both schema and prompt guidance; do not turn AGENTS output into general documentation.
- Do not introduce `shell:true`, cwd escape paths, unbounded stdout/stderr, synchronous long-running subprocesses on agent-facing paths, or fake-success fallbacks after CLI failure.
- Do not make slash commands mutate the project directly, bypass the agent turn, omit `customType`/`details`, or rely on UI notification when `sendMessage` is available.

## Update When
- Update when adding or changing `.omp/extensions/*.ts` command names, tool schemas, argument parsing, aliases, or completions.
- Update when adding or changing a subprocess-backed tool, cwd parameter, timeout/output limit, or wrapper around AOC/CodeGraph/Mind/search CLIs.
- Update when adding or changing slash commands, workflow prompts, `customType` names, or sendMessage details payloads.
- Update when adding tool actions, changing promptGuidelines, or broadening a wrapper from read-only/dry-run to a mutating capability.

```

Evidence:
- `.aoc/dox/map.json` — Marks `.omp/extensions` as `coverage: insufficient`, `status: candidate_local_agents`, with resolved chain `["AGENTS.md"]`.
- `AGENTS.md` — Root rules cover repo-wide AOC workflow and do not define ExtensionAPI, TypeBox schemas, sendMessage/customType, or subprocess-wrapper invariants.

Verification:
- `bun --check .omp/extensions/<changed-extension>.ts`

### `bin/AGENTS.md`
Scope: `bin`
Purpose: critic-approved compressed create: `bin` is root-only/insufficient in `.aoc/dox/map.json`; local rules protect public PATH entrypoints and generated/cache boundaries not covered by

```md
# Repository Guidelines

Scope: `bin`

## Local Contracts
- Preserve `bin/*` as public PATH entrypoints: Bash wrappers keep shebangs, `set -euo pipefail`, `exec` handoff on delegation, and existing colocated/repo-relative command resolution before PATH fallback.
- Do not hand-edit generated or managed outputs/cache under `bin`; update the source/template/regeneration path instead, and never treat `bin/__pycache__/*.pyc` as source.
- For Python CLIs in `bin`, keep argument parsing and runtime work behind `main()` and `if __name__ == "__main__"`; imports should not start services, clean processes, mutate files, or perform network work.

## Verification
- `PYTHONDONTWRITEBYTECODE=1 python3 -B bin/<changed-python-cli> --help >/dev/null`
- `bash -n bin/<changed-bash-wrapper>`
- `bash -n bin/aoc-context bin/aoc-hyperframes bin/aoc-init`
- `bin/<changed-command> --help >/dev/null`

## Do Not
- Do not create a local AGENTS file inside `bin/__pycache__`, review `.pyc` files as source, or patch generated/managed files without identifying the source generator or asset marker.
- Do not perform service startup, process cleanup, filesystem mutation, or network work at import time; do not hide CLI behavior outside parser/main entrypoints.
- Do not replace `exec` dispatch with plain subprocess calls, remove strict shell mode from Bash wrappers, simplify existing resolution to PATH-only lookup, or remove local/debug/release fallbacks without preserving the development workflow.

## Update When
- Update when adding or changing Python CLI files in `bin`, parser setup, or importable helper modules.
- Update when adding or changing a public command in `bin`, wrapper delegation, local binary lookup order, or help/startup behavior.
- Update when generated/managed markers move, regeneration ownership changes, or cache/source boundaries under `bin` change.

```

Evidence:
- `.aoc/dox/map.json` — Marks `bin` as `coverage: insufficient`, `status: candidate_local_agents`, with resolved chain `["AGENTS.md"]`.
- `AGENTS.md` — Root rules cover repo-wide AOC workflow and do not define `bin` wrapper, dispatcher, generated/cache, or Python CLI entrypoint invariants.

Verification:
- `bash -n bin/<changed-bash-wrapper>`
- `bin/<changed-command> --help >/dev/null`
- `bash -n bin/aoc-context bin/aoc-hyperframes bin/aoc-init`
- `PYTHONDONTWRITEBYTECODE=1 python3 -B bin/<changed-python-cli> --help >/dev/null`

### `crates/aoc-agent-wrap-rs/src/AGENTS.md`
Scope: `crates/aoc-agent-wrap-rs/src`
Purpose: critic-approved compressed create: root AGENTS does not cover Pulse wire shape, secret-safe child env, redaction, stop escalation, or detached Insight state.

```md
# Repository Guidelines

Scope: `crates/aoc-agent-wrap-rs/src`

## Local Contracts
- Route Pulse insight commands through `build_pulse_command_response`/`InsightCommand`: validate target agent, return JSON `command_result`s, and emit runtime/detached `PulseUpdate`s for state changes.
- Spawn Mind/Insight child processes with the existing `env_clear` allowlist path (`configure_mind_child_std_command_env` or equivalent); test any added allowed env key.
- Sanitize/redact external output before it reaches agent-visible status, telemetry, command results, exports, or persisted text; extend secret-pattern tests for new formats.
- Preserve stop/cancel semantics: SIGINT/INT grace, TERM grace, final kill, persisted detached cancellation, and parent/child job cancellation together.
- Insight dispatch must fail deterministically when manifests or subprocess config are absent: return bounded fallback/error result objects, not panics or live `.pi` assumptions.

## Verification
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml mind_child_env_excludes_ambient_secrets_and_keeps_allowlisted_vars`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml pulse_insight_dispatch_returns_structured_result`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml sanitize_activity_line_redacts_common_secret_patterns`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml stop_tokio_child_escalates_when_sigint_ignored`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6153-6163 marks this source path insufficient/candidate_local_agents with only AGENTS.md in the chain.
- `crates/aoc-agent-wrap-rs/src/main.rs` — symbol=build_pulse_command_response / InsightCommand / PulseUpdate::InsightRuntime; Insight commands are target-checked, parsed through InsightCommand, serialized to structured command_result JSON, and return runtime/detached PulseUpdate values.
- `crates/aoc-agent-wrap-rs/src/insight_orchestrator.rs` — symbol=InsightSupervisor / DetachedInsightRuntime; Covers configured child env, deterministic fallback/error results, output truncation, persisted detached jobs, cancellation, and child-job cancellation.

Verification:
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml sanitize_activity_line_redacts_common_secret_patterns`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml mind_child_env_excludes_ambient_secrets_and_keeps_allowlisted_vars`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml pulse_insight_dispatch_returns_structured_result`
- `cargo test --manifest-path crates/aoc-agent-wrap-rs/Cargo.toml stop_tokio_child_escalates_when_sigint_ignored`

### `crates/aoc-cli/AGENTS.md`
Scope: `crates/aoc-cli`
Purpose: critic-approved compressed create: package owns the user-facing `aoc` binary surface plus stateful Taskmaster, DOX, and map writes.

```md
# Repository Guidelines

Scope: `crates/aoc-cli`

## Local Contracts
- Add/change user-facing `aoc` commands only through `main.rs::Commands` and the existing module `handle_*_command` dispatch; preserve public names and aliases such as `map`/`see` unless fully migrated.
- State-mutating commands must use existing project-root/path/write helpers for Taskmaster, DOX, and map outputs; do not hand-roll writes to `.taskmaster/*`, `.aoc/dox/*`, or `.aoc/map/*`.
- Keep DOX review/apply conservative: approvals need evidence plus safe verification, verification commands pass `validate_verification_command`, and AGENTS writes stay dry-run/`--yes` guarded with unmanaged-content protection.

## Verification
- `cargo test -p aoc-cli`
- `cargo test -p aoc-cli dox`
- `cargo test -p aoc-cli map`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6166-6175 marks crates/aoc-cli insufficient/candidate_local_agents with only AGENTS.md inherited.
- `crates/aoc-cli/src/main.rs` — symbol=Commands / main; Top-level subcommands are defined in Commands; map has alias see; main dispatches each command to a module handle_*_command function.
- `crates/aoc-cli/src/dox.rs` — symbol=handle_apply / validate_verification_command; Apply supports dry-run, refuses writes without --yes, rejects unmanaged AGENTS.md content, and validates destructive verification command tokens.

Verification:
- `cargo test -p aoc-cli dox`
- `cargo test -p aoc-cli map`
- `cargo test -p aoc-cli`

### `crates/aoc-core/src/AGENTS.md`
Scope: `crates/aoc-core/src`
Purpose: critic-approved compressed create: subtree exports shared serde/wire/storage contracts where compatibility, framing, redaction, and budget constants are durable local invariants.

```md
# Repository Guidelines

Scope: `crates/aoc-core/src`

## Local Contracts
- Treat exported serde structs/enums as wire/storage contracts: preserve `rename_all`, tagged layouts, schema defaults, and backward-compatible `#[serde(default)]` unless a versioned migration and compatibility tests land together.
- Preserve Pulse IPC framing: newline-delimited JSON, `ProtocolVersion::CURRENT`, `DEFAULT_MAX_FRAME_BYTES`, oversize rejection, and decoder recovery after malformed frames.
- Do not persist or emit unredacted Mind event secrets; new `RawEventBody`/attrs text must use the sanitizer and keep `mind_sanitized` / `mind_sanitized_reasons` plus deterministic canonical JSON/hash behavior.
- Changes to consultation caps, T0/T1/T2 constraints, context-layer precedence, or overseer command policy require behavior tests for truncation/defaulting/error/allow-confirm-deny branches.

## Verification
- `cargo test -p aoc-core consultation_contracts::tests`
- `cargo test -p aoc-core mind_contracts::tests`
- `cargo test -p aoc-core mind_contracts::tests::sanitizer_redacts_message_and_nested_payload_secrets`
- `cargo test -p aoc-core pulse_ipc::tests`
- `cargo test -p aoc-core session_overseer::tests`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6238-6247 marks crates/aoc-core/src insufficient/candidate_local_agents with only AGENTS.md inherited.
- `crates/aoc-core/src/pulse_ipc.rs` — symbol=WireMsg / ProtocolVersion / DEFAULT_MAX_FRAME_BYTES; Pulse IPC uses tagged serde wire messages, protocol versioning, bounded newline-delimited JSON frames, oversize rejection, and decoder recovery tests.
- `crates/aoc-core/src/mind_contracts.rs` — symbol=sanitize_raw_event_for_storage / canonical_json / T0/T1/T2 tests; Mind contracts define sanitizer entrypoints, sanitized metadata markers, deterministic canonical JSON/hash behavior, and T0/T1/T2/context-pack behavior tests.

Verification:
- `cargo test -p aoc-core pulse_ipc::tests`
- `cargo test -p aoc-core mind_contracts::tests::sanitizer_redacts_message_and_nested_payload_secrets`
- `cargo test -p aoc-core consultation_contracts::tests`
- `cargo test -p aoc-core mind_contracts::tests`
- `cargo test -p aoc-core session_overseer::tests`

### `crates/aoc-hub-rs/src/AGENTS.md`
Scope: `crates/aoc-hub-rs/src`
Purpose: critic-approved compressed create: root rules do not cover hub protocol/session/UDS transport invariants.

```md
# Repository Guidelines

Scope: `crates/aoc-hub-rs/src`

## Local Contracts
- Validate hub envelopes before registration or routing: protocol version, required fields, RFC3339 timestamp, session id, role, and publisher identity must pass first; publisher ids stay scoped as `{session_id}::{pane_or_agent}`. Routing/consultation changes must cover accepted route+ack and invalid-target error behavior.
- Keep transport bounds and observability constraints: WebSocket envelopes, patches, file lists, and UDS frames stay capped by the existing constants, and raw message bodies remain debug-only.
- Preserve private UDS lifecycle: create socket parents with user-private permissions, remove stale sockets before bind, bind sockets private to the user, and remove the socket path on shutdown.

## Verification
- `cargo test -p aoc-hub-rs --all-targets command_errors_include_code_and_message`
- `cargo test -p aoc-hub-rs --all-targets snapshot_on_connect_and_ordered_deltas`
- `cargo test -p aoc-hub-rs --all-targets stop_agent_command_routes_and_acks`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6262-6270 shows root-only resolved chain and insufficient coverage.
- `crates/aoc-hub-rs/src/main.rs` — symbol=WebSocket validation / payload limits; Validates hello/session/publisher before registering and validates later frames before handling; enforces payload limits and debug-only raw logging.
- `crates/aoc-hub-rs/src/pulse_uds.rs` — symbol=UDS lifecycle / routing tests; Handles private UDS directory/socket setup, stale removal, cleanup, frame limits, publisher scoping, target validation, and routing/error tests.

Verification:
- `cargo test -p aoc-hub-rs --all-targets stop_agent_command_routes_and_acks`
- `cargo test -p aoc-hub-rs --all-targets command_errors_include_code_and_message`
- `cargo test -p aoc-hub-rs --all-targets snapshot_on_connect_and_ordered_deltas`

### `crates/aoc-installer/src/AGENTS.md`
Scope: `crates/aoc-installer/src`
Purpose: critic-approved compressed create: root rules do not protect live installer side effects, downloader/source validation, or host PATH process spawning.

```md
# Repository Guidelines

Scope: `crates/aoc-installer/src`

## Local Contracts
- Treat this as live installer code: routine verification must not run installs, downloaded `install.sh`, `--yes`, post-install doctor, or mutate real `~/.local/bin`/`~/.config`; prefer parser/resolver checks, compile checks, and no-run tests.
- Keep installs explicit, interactive by default, and user-local in messaging. Keep downloads constrained to GitHub archive/API flows and slug-shaped repos; if touching explicit `--repo`, close the current validation seam instead of widening accepted inputs.
- Tests or refactors around command spawning/downloading must isolate temp PATH/filesystem fixtures and target pure helpers before integration paths; never test by executing downloaded code or depending on host `curl`/`wget`/`tar`/`bash`/`git`/`aoc-doctor` behavior.

## Verification
- `cargo check -p aoc-installer`
- `cargo test -p aoc-installer --no-run`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6286-6294 shows root-only resolved chain and insufficient coverage.
- `crates/aoc-installer/src/main.rs` — symbol=run / resolve_repo / downloader helpers; Downloads a GitHub archive, extracts with tar, runs downloaded install.sh, optionally runs doctor, keeps --yes explicit, constrains auto-detected repos/remotes, and exposes pure-ish parsing/source-root helpers plus host process spawning/PATH detection.

Verification:
- `cargo check -p aoc-installer`
- `cargo test -p aoc-installer --no-run`

### `crates/aoc-mind/src/AGENTS.md`
Scope: `crates/aoc-mind/src`
Purpose: critic-approved compressed create: root rules do not cover Mind state layout, legacy compatibility, file-lock/store-lease coordination, deterministic fallback, manifests, watermark

```md
# Repository Guidelines

Scope: `crates/aoc-mind/src`

## Local Contracts
- Treat project Mind state layout and compatibility seams as stable API: derive runtime/store/legacy/lock/health paths through `MindProjectPaths` and resolver helpers, sanitize project/session/pane path components, and keep legacy imports/readers plus `AOC_MIND_FEED_COMPAT`, `AOC_PI_SESSION_DIR`, and `AOC_PI_SETTINGS_PATH` intentional.
- Preserve runtime coordination as dual ownership: service/reflector/T3 work requires the advisory file lock plus the store lease before claiming jobs, lock conflicts are not claims, and service ticks keep heartbeat/health snapshots current.
- Preserve deterministic provenance through ingestion, observer fallback, retrieval, T3, and finalization: semantic/guardrail failures fall back deterministically, export manifests keep schema/slice/artifact/tag/watermark/T3 fields, and watermarks/T3 backlog jobs advance only with slice provenance.

## Verification
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib explicit_overrides_and_legacy_paths_are_supported`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib guardrail_budget_exceeded_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib latest_pi_session_file_prefers_newest_jsonl`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib project_paths_match_expected_layout`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_scope_and_lease_queries`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_tick_health_and_observer_effects`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib semantic_failure_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib sync_session_file_into_project_store_ingests_pi_jsonl`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6310-6330 shows root-only resolved chain and insufficient coverage for `src`/`src/bin`.
- `crates/aoc-mind/src/standalone.rs` — symbol=MindProjectPaths / MindServiceLeaseGuard / Pi session sync; Defines project layout, resolvers, legacy import, Pi session discovery/sync, service leases, health snapshots, and lock paths.
- `crates/aoc-mind/src/lib.rs` — symbol=SessionExportManifest / finalization / T3 enqueue; Defines export manifest fields, host plan files, T3 enqueue, artifact selection, and watermark scope.

Verification:
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib project_paths_match_expected_layout`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib explicit_overrides_and_legacy_paths_are_supported`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib sync_session_file_into_project_store_ingests_pi_jsonl`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib latest_pi_session_file_prefers_newest_jsonl`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_scope_and_lease_queries`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib runtime_owns_tick_health_and_observer_effects`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib semantic_failure_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib guardrail_budget_exceeded_falls_back_to_deterministic_t1`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`

### `crates/aoc-mind/src/bin/AGENTS.md`
Scope: `crates/aoc-mind/src/bin`
Purpose: critic-approved compressed create: binary-specific machine output, exit-code, long-running loop, finalization write-order, and project-scoped external memory checks remain additive

```md
# Repository Guidelines

Scope: `crates/aoc-mind/src/bin`

## Local Contracts
- Keep `aoc-mind-service` project-root scoped and path/store construction delegated to library APIs (`MindProjectPaths`, `open_project_store`, `MindRuntimeCore`); do not duplicate Mind path derivation in the binary.
- Preserve CLI machine contracts: JSON mode emits structured JSON without human prose on stdout, human errors go to stderr, success exits `0`, operational failures exit `1`, and existing parse/mode validation failures continue to exit `2`.
- Long-running `serve`/`watch-pi` paths must compose existing single-tick/sync helpers, keep service heartbeat/queue health updates, and retain minimum sleep clamps to avoid busy loops.
- Finalization writes remain safety ordered: validate every prepared export file with `ensure_safe_export_text`, write only `prepared.host_plan.export_files`, and advance the project watermark only after all writes succeed.
- Doctor memory checks stay project-scoped: external `aoc-mem` calls run with `current_dir(project_root)` and validate path/header consistency before reporting healthy memory.

## Verification
- `cargo run --manifest-path crates/aoc-mind/Cargo.toml --bin aoc-mind-service -- status --project-root /tmp/aoc-mind-dox-smoke --json`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --bins --no-run`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json:6322-6330 shows root-only resolved chain and insufficient coverage for `src/bin`.
- `crates/aoc-mind/src/bin/aoc-mind-service.rs` — symbol=CLI dispatch / serve / finalize / doctor memory; Defines project-root commands, JSON/human stdout-stderr and exit-code behavior, serve/watch loops, safety-ordered finalization writes, and project-scoped `aoc-mem` checks.

Verification:
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --bins --no-run`
- `cargo run --manifest-path crates/aoc-mind/Cargo.toml --bin aoc-mind-service -- status --project-root /tmp/aoc-mind-dox-smoke --json`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib prepare_session_finalize_execution_builds_host_plan_and_enqueues_t3`
- `cargo test --manifest-path crates/aoc-mind/Cargo.toml --lib session_export_bundle_renders_markdown_and_manifest`

### `crates/aoc-mission-control/src/AGENTS.md`
Scope: `crates/aoc-mission-control/src`
Purpose: critic-approved compressed create: Mission Control has Pulse sequencing/session filters, config/env parsing, offline fallback rendering, Zellij vs standalone launch paths, and Mind

```md
# Repository Guidelines

Scope: `crates/aoc-mission-control/src`

## Local Contracts
- Preserve Pulse hub ordering/resync semantics: ignore other-session or newer-protocol envelopes, drop stale deltas, reconnect on sequence gaps, and clear cached hub state before resync.
- Keep Mission Control runtime knobs in config.rs; new AOC_* env vars must use existing bool parsing/default conventions and clamp user-controlled refresh/poll intervals.
- Baseline rendering must not require a live Pulse hub or Zellij polling; local snapshot/presence fallback must still work when Pulse is disabled/offline.
- Keep Zellij in-session launch/navigation distinct from standalone aoc-launch/aoc-new-tab fallbacks; worker launch plans use program/args/env/cwd with Command::new, not shell-expanded strings.
- Mind consultation persistence must keep provenance, task/file links, and stable prompt/source identifiers, not display-only summaries.

## Verification
- `cargo test --manifest-path crates/aoc-mission-control/Cargo.toml`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json marks this source path coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-mission-control/src/hub.rs` — symbol=HubEvent / Pulse sequencing; Connects to Pulse, sends Hello/Subscribe, filters session/protocol, drops stale deltas, and reconnects on sequence gaps.
- `crates/aoc-mission-control/src/ops.rs` — symbol=WorkerLaunchPlan; Builds worker launch plans as program/args/env/cwd, using aoc-new-tab in Zellij and aoc-launch otherwise, then executes via Command::new.

Verification:
- `cargo test --manifest-path crates/aoc-mission-control/Cargo.toml`

### `crates/aoc-opencode-adapter/src/AGENTS.md`
Scope: `crates/aoc-opencode-adapter/src`
Purpose: critic-approved compressed create: append-only NDJSON ingestion, redaction, deterministic identity, lineage compatibility, and restart attribution are adapter-specific invariants a

```md
# Repository Guidelines

Scope: `crates/aoc-opencode-adapter/src`

## Local Contracts
- Conversation files are append-only but truncation-tolerant: checkpoint raw byte cursors, defer incomplete trailing lines, skip corrupt complete lines, and advance only to consumed complete records.
- Never persist raw tool output into Mind; sanitize RawEvent before insert and keep tool-result output redacted through compaction/normalization.
- Event identity and fallback timestamps stay deterministic: prefer event_id/id, otherwise hash conversation_id + line_offset + canonical JSON; use line-offset fallback timestamps only when source timestamps are missing/invalid.
- Maintain lineage compatibility across mind_lineage, lineage, conversation_lineage, payload lineage, and legacy parent/root key spellings; emit canonical lineage attrs when session_id is present.
- Task attribution must resume from latest_context_state and update on tm/aoc-task lifecycle signals across initial and resumed ingest.

## Verification
- `cargo test --manifest-path crates/aoc-opencode-adapter/Cargo.toml`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json marks this source path coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-opencode-adapter/src/lib.rs` — symbol=OpenCode ingest / normalize / attribution; Reads raw checkpoint cursors, handles truncation/partial/corrupt lines, sanitizes raw events before storage, redacts tool output, preserves deterministic ids/timestamps, lineage compatibility, and task context resume.

Verification:
- `cargo test --manifest-path crates/aoc-opencode-adapter/Cargo.toml`

### `crates/aoc-pi-adapter/src/AGENTS.md`
Scope: `crates/aoc-pi-adapter/src`
Purpose: critic-approved compressed create: Pi session header identity, cursor semantics, redaction, source attrs/lineage, and compaction rebuildability are durable adapter invariants.

```md
# Repository Guidelines

Scope: `crates/aoc-pi-adapter/src`

## Local Contracts
- Require a newline-terminated JSON session header and derive conversation_id as pi:<session_id>; keep missing-id fallback based on the session file path stable.
- Ingest only complete newline-delimited entries after header/checkpoint cursor; never ingest the header, defer partial trailing lines, skip corrupt complete lines, and reset to header_end_cursor on truncation.
- Never persist bash/tool output from Pi sessions into Mind; ToolResultEvent.output stays None and redacted while source output remains only in the Pi session/artifact file.
- Preserve Pi source attrs and lineage attrs on raw events: session/file/conversation/import ids, entry id/type/parent, cwd/version/parent session when present, and LINEAGE_ATTRS_KEY with root conversation id.
- Compaction imports must keep checkpoint/T0 slice rebuildability: marker/source event links, entry ids, source/read/modified files, tokens, first-kept entry, and pi_compaction_checkpoint source remain round-trippable.

## Verification
- `cargo test --manifest-path crates/aoc-pi-adapter/Cargo.toml`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json marks this source path coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-pi-adapter/src/lib.rs` — symbol=Pi session ingest / source attrs / compaction; Requires newline-terminated session headers, handles cursors/truncation/partial/corrupt lines, drops tool output, preserves source and lineage attrs, and stores compaction checkpoint/T0 slice fields.

Verification:
- `cargo test --manifest-path crates/aoc-pi-adapter/Cargo.toml`

### `crates/aoc-segment-routing/src/AGENTS.md`
Scope: `crates/aoc-segment-routing/src`
Purpose: critic-approved compressed create: routing precedence/provenance, uncertainty fallback, and manual override audit semantics are compact and operational.

```md
# Repository Guidelines

Scope: `crates/aoc-segment-routing/src`

## Local Contracts
- SegmentRouter::compute_auto_route must prefer a non-empty active Taskmaster tag mapped by tag_to_segment over heuristics, emit RouteOrigin::Taskmaster, use Taskmaster confidence, and keep taskmaster_tag_map...source=context_state provenance.
- Heuristic routing must use default_uncertain_segment for low-confidence or ambiguous top candidates; uncertain_fallback keeps useful secondary candidates and includes the normalized default_global_segment fallback when absent.
- Manual overrides must reject empty patch_id/primary segment, normalize and dedupe segments case-insensitively, cap secondaries, preserve prior auto route candidates when possible, set ManualOverride/overridden_by, and include override_patch plus base provenance.

## Verification
- `cargo test -p aoc-segment-routing --lib`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json marks this source path coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-segment-routing/src/lib.rs` — symbol=SegmentRouter::compute_auto_route / compute_heuristic_route / apply_override; Implements Taskmaster tag precedence, uncertain fallback behavior, manual override normalization/deduping/caps, overridden_by, and override/base provenance.

Verification:
- `cargo test -p aoc-segment-routing --lib`

### `crates/aoc-storage/src/AGENTS.md`
Scope: `crates/aoc-storage/src`
Purpose: critic-approved compressed create: storage schema/versioning, secret rejection, lease ownership, segment-route replacement, and compaction round trips are durable SQLite boundary i

```md
# Repository Guidelines

Scope: `crates/aoc-storage/src`

## Local Contracts
- Schema changes must be monotonic and versioned: bump MIND_SCHEMA_VERSION, extend MindStore::migrate in order, set PRAGMA user_version, record migrations where current paths do, and keep explicit SELECT lists/parsers/round-trip tests synchronized.
- Storage boundaries must reject unredacted secrets: raw events use raw_event_contains_unredacted_secret, and text-bearing durable surfaces use ensure_no_secrets_in_text/optional variants before INSERT/UPSERT.
- Reflector/T3 leases and job claims remain owner- and expiry-gated: acquisition replaces only same-owner or expired leases, and claim_next_* returns None unless owner_id matches and expires_at >= now.
- Segment-route persistence preserves replacement semantics: delete old rows before replacement, load ordered by confidence then segment id, error on invalid confidence/origin, and strip storage rank suffixes from public reasons.
- Compaction checkpoint/T0 slice storage must preserve idempotent upserts, conversation-scoped compaction_entry_id, latest lookups by conversation/session/checkpoint, and round-trippable slice hashes/source/read/modified/token/first-kept fields.

## Verification
- `cargo test -p aoc-storage --lib`

```

Evidence:
- `.aoc/dox/map.json` — .aoc/dox/map.json marks this source path coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-storage/src/lib.rs` — symbol=MindStore::migrate / secret guards / leases / segment routes / compaction storage; Implements schema versioned migrations, storage-boundary secret rejection, owner/expiry-gated leases and claims, segment-route replacement/load semantics, and compaction checkpoint/T0 slice round trips.

Verification:
- `cargo test -p aoc-storage --lib`

### `crates/aoc-task-attribution/src/AGENTS.md`
Scope: `crates/aoc-task-attribution/src`
Purpose: critic-approved compressed create: root AGENTS does not cover Mind artifact-task confidence, provenance, dedup, or extraction boundaries.

```md
# Repository Guidelines

Scope: `crates/aoc-task-attribution/src`

## Local Contracts
- Preserve artifact-task link meaning: `Active`, `Mentioned`, `WorkedOn`, completion-backfilled `WorkedOn`, and `Completed` keep their confidence order/source strings; duplicate `(task_id, relation)` drafts merge via `LinkDraft::key`/`upsert_draft` with highest confidence/source and unioned sorted evidence.
- Keep attribution inputs narrow and evidence-backed: task IDs may come only from active context states, artifact text, and t0 compact events inside `AttributionConfig`'s mention window; evidence IDs retain `ctx:`, `artifact:*:text`, or `t0:` prefixes.

## Verification
- `cargo test --manifest-path crates/Cargo.toml -p aoc-task-attribution --lib`

## Do Not
- Do not add fuzzy/global task matching, widen mention windows, or change completion backfill/Completed relation timing without focused attribution tests.
- Do not bypass sorted `BTreeSet` evidence collection before `ArtifactTaskLink::new`.

## Update When
- Confidence constants, `TaskAttributionEngine::attribute_conversation`, completion/mention extraction, draft merge keys, or store upsert flow change.

```

Evidence:
- `.aoc/dox/map.json` — crates/aoc-task-attribution/src is source coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-task-attribution/src/lib.rs` — symbol=TaskAttributionEngine::attribute_conversation / upsert_draft / extract_task_ids; Defines confidence constants, relation link creation, completion signals, t0/artifact mention extraction, draft merging, sorted evidence, and bounded task id extraction.
- `crates/aoc-task-attribution/Cargo.toml` — Package name supports the scoped verification command.

Verification:
- `cargo test --manifest-path crates/Cargo.toml -p aoc-task-attribution --lib`

### `crates/aoc-taskmaster/src/AGENTS.md`
Scope: `crates/aoc-taskmaster/src`
Purpose: critic-approved compressed create: root AGENTS does not cover the TUI writer path, root resolution, terminal restoration, and watcher bounds.

```md
# Repository Guidelines

Scope: `crates/aoc-taskmaster/src`

## Local Contracts
- Taskmaster mutations must update visible `App` state and the `ProjectData` mirror before persisting only through `save_project -> write_atomic -> touch_state_file`; keep `tasks.json` as pretty `ProjectData` JSON and `state.json` updates on that path.
- Root resolution stays non-creating: `AOC_TASKMASTER_ROOT`/`TM_ROOT`/`TASKMASTER_ROOT` must canonicalize to existing directories; otherwise use the nearest existing Taskmaster root, then `AOC_PROJECT_ROOT` only if it is already a Taskmaster root, else cwd.
- TUI runtime safety is part of the contract: restore raw mode, alternate screen, mouse capture, and cursor visibility after `run_app`; keep refresh watching non-recursive and bounded to `.taskmaster`, `.taskmaster/tasks`, or the root fallback with bounded signaling.

## Verification
- `cargo check --manifest-path crates/Cargo.toml -p aoc-taskmaster`

## Do Not
- Do not create `.taskmaster` from root detection, persist subtask `aocPrd`, remove legacy `parse_project_compat` formats, add recursive/project-wide watchers, or skip terminal restore after setup.
- Do not write `.taskmaster/tasks/tasks.json` or `.taskmaster/state.json` directly from handlers, or update `App.tasks` without matching `project.tags` changes.

## Update When
- `resolve_root*`, `find_taskmaster_root`, `is_taskmaster_root`, mutation handlers, `save_project`, `touch_state_file`, `write_atomic`, `parse_project_compat`, `validate_project`, `main`, terminal setup/restore, `run_app`, or `setup_watcher` change.

```

Evidence:
- `.aoc/dox/map.json` — crates/aoc-taskmaster/src is source coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-taskmaster/src/state.rs` — symbol=resolve_root* / mutation handlers / save_project / write_atomic / parse_project_compat / validate_project; Covers root resolution, visible App state and ProjectData mirroring, atomic pretty ProjectData writes, state touches, compatibility parsing, and unsupported subtask aocPrd validation.
- `crates/aoc-taskmaster/src/main.rs` — symbol=setup_terminal / restore_terminal / run_app / setup_watcher; Covers terminal setup/restore and bounded non-recursive watcher behavior.

Verification:
- `cargo check --manifest-path crates/Cargo.toml -p aoc-taskmaster`

### `crates/aoc-yazi-mermaid/src/AGENTS.md`
Scope: `crates/aoc-yazi-mermaid/src`
Purpose: critic-approved compressed create: root AGENTS has no Yazi preview CLI, stdout, cache identity, atomic render, or Markdown fence contract.

```md
# Repository Guidelines

Scope: `crates/aoc-yazi-mermaid/src`

## Local Contracts
- Keep the Yazi CLI machine-readable: success prints exactly the cache PNG path to stdout, diagnostics/errors go to stderr, and failures exit nonzero; preserve `--input`, `--cache-dir`, `--cols`, `--rows`, `--block-index`, and `--theme` semantics with the preview integration.
- Cache/render identity is part of behavior: cache keys include package version, canonical input path, file length/mtime, block index, cols, rows, and theme; a cache hit must be a non-empty file; rendering writes a temp PNG and renames only after PNG encoding succeeds.
- Markdown inputs select the requested Mermaid block from ```mermaid, ~~~mermaid, or :::mermaid fences; missing block indexes are errors, and non-Markdown files are raw Mermaid source.

## Verification
- `cargo test --manifest-path crates/Cargo.toml -p aoc-yazi-mermaid`

## Do Not
- Do not add progress/debug/status text to stdout, convert render/load failures into successful output, reuse stale/empty cache files, write the final PNG directly, or fall back to another Mermaid block when `block_index` is missing.

## Update When
- `Args`, `main`, `run`, `cache_path_for`, `render_preview`, `is_nonempty_file`, `load_mermaid_source`, fence detection, render sizing/theme, or stdout/error handling change.

```

Evidence:
- `.aoc/dox/map.json` — crates/aoc-yazi-mermaid/src is source coverage=insufficient, status=candidate_local_agents, resolved_agents_chain=[AGENTS.md].
- `crates/aoc-yazi-mermaid/src/main.rs` — symbol=Args / main / run / cache_path_for / render_preview / load_mermaid_source; Defines Yazi CLI flags, stderr/nonzero failures, stdout cache path success, cache key identity, temp PNG rename after encoding, nonempty cache check, and Mermaid fence extraction.
- `crates/aoc-yazi-mermaid/Cargo.toml` — Package name supports the scoped verification command.

Verification:
- `cargo test --manifest-path crates/Cargo.toml -p aoc-yazi-mermaid`

## Operator apply command
After reviewing this packet, apply manually with:

```bash
aoc dox apply --yes
```