# Documentation map

Start with the document that owns the fact you need. Code and executable checks establish implemented behavior; roadmap proposals and dated measurements describe their stated scope and capture date.

| Need | Canonical document |
| --- | --- |
| Build, run, project overview | [README](../README.md) |
| Making a game from the template | [Getting started](getting-started.md) |
| Repository rules | [AGENTS](../AGENTS.md); [renderer rules](../crates/tungsten-render/AGENTS.md) when editing that crate |
| Change-to-check matrix, targeted iteration and read-only selection | [Validation](validation.md) |
| Task → source paths | [LLM index](LLM_INDEX.md) |
| Current architecture and reload support | [Design](../DESIGN.md) |
| Design rationale | [Decision index](DECISION_INDEX.md), then one `D-NNN` section of [DECISIONS](../DECISIONS.md) |
| Open findings, recorded limits and follow-ups | [Known issues](known-issues.md) |
| Plan headers, lifecycle and archival | [Plan rules](plans/README.md) |
| Agent discovery, skills and filters | [Agent setup](agent-setup.md) |
| Performance capture and comparison rules | [Profiling workflow](perf/profiling-workflow.md) |
| Workloads, knobs, dated numbers and performance proposals | [Benchmarks](perf/benchmarks.md) |
| Release preparation, checks, handoff and recovery | [Release procedure](releases.md) |
| Platformer asset/layout authoring | [Authoring guide](../examples/01_platformer/tools/README.md) |
| The template's sounds, sprites and Windows icon: scripts and provenance | [Template tools](../templates/basic/tools/README.md) |
| Asset types, manifest sections and coverage exceptions | [Assets](assets.md) |
| Font inventory and registered IDs | [Fonts](../assets/fonts/README.md) |
| Test fixture provenance | [GPU fixtures](../examples/02_bench/tests/fixtures/README.md), [audio fixtures](../crates/tungsten-core/tests/fixtures/audio/README.md) |
| Dated releases and historical paths | [Changelog](../CHANGELOG.md) |

For large docs, locate a heading with `rg -n '^#{1,3} ' <file>` and read the relevant section. Check current source before applying an old benchmark result or proposed API. Keep operational rules in AGENTS, rationale in decisions, and execution steps in active plans; skills route to those sources instead of copying their tables.

Completed or retired plans live under `docs/plans/archive/` with their original basenames. Historical `docs/plans/<name>.md` references may therefore name their former locations. Agents never read, search or list the archive.

After documentation changes, run `just ctx`, `just repo-check` and `git diff --check`; substantial work also finishes with `just check`. The checker covers maintained docs and active program documents one folder deep, including local destinations and Markdown fragments; archive targets stay unread. Check other touched local links/anchors and external URLs separately.
