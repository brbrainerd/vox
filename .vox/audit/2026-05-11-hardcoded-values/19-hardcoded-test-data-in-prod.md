# 19 — hardcoded test data in prod

**Severity**: warning  
**Itemized**: 8

### hv-1386 — `crates/vox-actor-runtime/src/notify.rs:148`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-actor-runtime/src/notify.rs"`

**Confidence**: medium

---

### hv-1387 — `crates/vox-actor-runtime/src/notify.rs:174`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-actor-runtime/src/notify.rs"`

**Confidence**: medium

---

### hv-1388 — `crates/vox-actor-runtime/src/notify.rs:191`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-actor-runtime/src/notify.rs"`

**Confidence**: medium

---

### hv-1389 — `crates/vox-corpus/src/synthetic_gen/bodies/_routing_body.rs:226`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-corpus/src/synthetic_gen/bodies/_routing_body.rs"`

**Confidence**: medium

---

### hv-1390 — `crates/vox-git/src/object.rs:121`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-git/src/object.rs"`

**Confidence**: medium

---

### hv-1391 — `crates/vox-git/src/object.rs:123`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-git/src/object.rs"`

**Confidence**: medium

---

### hv-1392 — `crates/vox-orchestrator/src/pii_filter.rs:43`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-orchestrator/src/pii_filter.rs"`

**Confidence**: medium

---

### hv-1393 — `crates/vox-publisher/src/publication_preflight/tests.rs:93`

**Substring**

```text
@example.com
```

**Why it matters**: Example identities leak into prod code paths and confuse audits.

**Fix** (remove-test-data-from-prod): Remove example emails/user names from production paths; load fixtures only in tests.

**Verify**: `rg -nF "@example.com" "crates/vox-publisher/src/publication_preflight/tests.rs"`

**Confidence**: medium

---

