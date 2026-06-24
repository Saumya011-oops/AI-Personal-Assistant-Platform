# RAG Retrieval Evaluation Report

**Overall Score**: 100.0%
**Total Queries**: 18
**Passed**: 18
**Failed**: 0

## Category Performance

| Category | Description | Score |
|---|---|---|
| Category A | Direct Keyword Matching | 100.0% |
| Category B | Dense Vector Semantic Search | 100.0% |
| Category C | Hybrid Search | 100.0% |
| Category D | Metadata-Faceted Filtering | 100.0% |
| Category E | Contextual Temporal Search | 100.0% |
| Category F | Recursive Document Multi-Hop Retrieval | 100.0% |
| Category G | Confidence Gating Canary | 100.0% |
| Category H | Ambiguity Routing | 100.0% |
| Category I | Citation & Source Integrity | 100.0% |
| Category J | Multi-Hop & Comparison Retrieval Precision | 100.0% |

## Query Execution Details

| ID | Category | Query | Status | Citations | Passed | Latency | Doc Recall | Entity Recall | Topic Recall | Reasons / Mismatch |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | A | "What is Grafana?" | OK | 3 | ✅ | 14675ms | 100.0% | 100.0% | 100.0% | None |
| A2 | A | "Explain authentication" | OK | 3 | ✅ | 4627ms | 100.0% | 100.0% | 100.0% | None |
| B1 | B | "Describe system telemetry and metrics" | OK | 3 | ✅ | 5295ms | 100.0% | 100.0% | 100.0% | None |
| C1 | C | "What chunk size does the RAG system use?" | OK | 3 | ✅ | 20053ms | 100.0% | 100.0% | 100.0% | None |
| D1 | D | "Show onboarding notes from Notion" | EMPTY_RETRIEVAL | 0 | ✅ | 2894ms | 100.0% | 100.0% | 100.0% | None |
| E1 | E | "What happened last week?" | EMPTY_RETRIEVAL | 0 | ✅ | 5097ms | 100.0% | 100.0% | 100.0% | None |
| F1 | F | "How does onboarding connect to Notion setup?" | OK | 3 | ✅ | 21206ms | 100.0% | 100.0% | 100.0% | None |
| F2 | F | "Compare Notion and Obsidian integrations" | OK | 3 | ✅ | 17331ms | 0.0% | 100.0% | 100.0% | Document recall below 100% (info only): expected all of ["onboarding_system_permissions"], got recall 0.00 |
| G1 | G | "Which database stores employee salaries?" | LOW_CONFIDENCE_RETRIEVAL | 0 | ✅ | 7012ms | 100.0% | 100.0% | 100.0% | None |
| G2 | G | "What is the CEO's personal email?" | LOW_CONFIDENCE_RETRIEVAL | 0 | ✅ | 9736ms | 100.0% | 100.0% | 100.0% | None |
| H1 | H | "Explain setup" | AMBIGUOUS_RETRIEVAL | 0 | ✅ | 9651ms | 100.0% | 100.0% | 100.0% | None |
| I1 | I | "Where are desktop credentials stored?" | OK | 3 | ✅ | 9035ms | 100.0% | 100.0% | 100.0% | None |
| J1 | J | "Compare Notion and Obsidian integrations" | OK | 3 | ✅ | 37619ms | 0.0% | 100.0% | 100.0% | Document recall below 100% (info only): expected all of ["onboarding_system_permissions"], got recall 0.00 |
| J2 | J | "Compare Prometheus and Grafana" | OK | 3 | ✅ | 7408ms | 100.0% | 100.0% | 100.0% | None |
| J3 | J | "How does onboarding connect to Notion setup?" | OK | 3 | ✅ | 22680ms | 100.0% | 100.0% | 100.0% | None |
| J4 | J | "How does authentication interact with Qdrant access control?" | OK | 3 | ✅ | 5200ms | 0.0% | 100.0% | 100.0% | Document recall below 100% (info only): expected all of ["authentication_flow_oauth2", "rag_architecture_overview"], got recall 0.00 |
| J5 | J | "Difference between OAuth and token management" | OK | 3 | ✅ | 17251ms | 100.0% | 100.0% | 100.0% | None |
| J6 | J | "Explain setup" | AMBIGUOUS_RETRIEVAL | 0 | ✅ | 3773ms | 100.0% | 100.0% | 100.0% | None |
