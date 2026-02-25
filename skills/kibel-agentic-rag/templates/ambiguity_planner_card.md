## Ambiguity Planner Card

Question:

- `<paste user question>`

### 1) Facet decomposition

- intent:
- target:
- artifact:
- time:
- scope:

### 2) Candidate query set (profile budget aware)

- anchor:
- artifact-focused:
- scope-focused:
- time-focused:
- verification-focused:

### 3) Expected evidence shape

- Must-have evidence:
- Nice-to-have evidence:
- Disqualifying noise patterns:

### 4) Corrective loop trigger

- Profile thresholds:
  - fast: `top5_relevance < 0.60` or `must_have_evidence_hits < 1`
  - balanced: `top5_relevance < 0.75` or `must_have_evidence_hits < 2`
  - deep: `top5_relevance < 0.85` or `must_have_evidence_hits < 2`
- Trigger if top5 relevance < profile threshold:
- Trigger if must-have evidence is missing (below profile threshold):
- Trigger if conflicting claims are found:

### 5) Corrective actions

1. Rewrite candidates using missing facet terms.
2. Narrow by filters (`group-id`, `folder-id`, `user-id`) if available.
3. Re-run retrieval within remaining budget.
