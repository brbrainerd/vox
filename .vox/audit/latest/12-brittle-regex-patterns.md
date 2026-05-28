# 12 — brittle regex patterns

**Severity**: info  
**Itemized**: 100

### hv-0871 — `contracts/code-audit/rules.v1.yaml:626`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/code-audit/rules.v1.yaml"`

**Confidence**: medium

---

### hv-0872 — `contracts/code-audit/rules.v1.yaml:633`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/code-audit/rules.v1.yaml"`

**Confidence**: medium

---

### hv-0873 — `contracts/reports/scaling-audit/findings-latest.json:5330`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0874 — `contracts/reports/scaling-audit/findings-latest.json:16294`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0875 — `contracts/reports/scaling-audit/findings-latest.json:16305`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0876 — `contracts/reports/scaling-audit/findings-latest.json:16317`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0877 — `contracts/reports/scaling-audit/findings-latest.json:16328`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0878 — `contracts/reports/scaling-audit/findings-latest.json:16340`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0879 — `contracts/reports/scaling-audit/findings-latest.json:16352`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0880 — `contracts/reports/scaling-audit/findings-latest.json:16364`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0881 — `contracts/reports/scaling-audit/findings-latest.json:16376`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0882 — `contracts/reports/scaling-audit/findings-latest.json:16388`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0883 — `contracts/reports/scaling-audit/findings-latest.json:17035`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0884 — `contracts/reports/scaling-audit/findings-latest.json:17243`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0885 — `contracts/reports/scaling-audit/findings-latest.json:17255`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0886 — `contracts/reports/scaling-audit/findings-latest.json:17290`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0887 — `contracts/reports/scaling-audit/findings-latest.json:17302`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0888 — `contracts/reports/scaling-audit/findings-latest.json:17314`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0889 — `contracts/reports/scaling-audit/findings-latest.json:17326`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0890 — `contracts/reports/scaling-audit/findings-latest.json:17338`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0891 — `contracts/reports/scaling-audit/findings-latest.json:18260`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0892 — `contracts/reports/scaling-audit/findings-latest.json:18385`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0893 — `contracts/reports/scaling-audit/findings-latest.json:18409`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0894 — `contracts/reports/scaling-audit/findings-latest.json:18421`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0895 — `contracts/reports/scaling-audit/findings-latest.json:18433`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0896 — `contracts/reports/scaling-audit/findings-latest.json:18445`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0897 — `contracts/reports/scaling-audit/findings-latest.json:18457`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0898 — `contracts/reports/scaling-audit/findings-latest.json:18469`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0899 — `contracts/reports/scaling-audit/findings-latest.json:18481`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0900 — `contracts/reports/scaling-audit/findings-latest.json:18493`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0901 — `contracts/reports/scaling-audit/findings-latest.json:18523`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0902 — `contracts/reports/scaling-audit/findings-latest.json:18535`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0903 — `contracts/reports/scaling-audit/findings-latest.json:18656`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0904 — `contracts/reports/scaling-audit/findings-latest.json:18668`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0905 — `contracts/reports/scaling-audit/findings-latest.json:19894`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0906 — `contracts/reports/scaling-audit/findings-latest.json:19906`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0907 — `contracts/reports/scaling-audit/findings-latest.json:20285`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0908 — `contracts/reports/scaling-audit/findings-latest.json:21526`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0909 — `contracts/reports/scaling-audit/findings-latest.json:21538`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0910 — `contracts/reports/scaling-audit/findings-latest.json:21550`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0911 — `contracts/reports/scaling-audit/findings-latest.json:21562`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0912 — `contracts/reports/scaling-audit/findings-latest.json:21574`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0913 — `contracts/reports/scaling-audit/findings-latest.json:21586`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0914 — `contracts/reports/scaling-audit/findings-latest.json:21610`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0915 — `contracts/reports/scaling-audit/findings-latest.json:21758`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0916 — `contracts/reports/scaling-audit/findings-latest.json:21850`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0917 — `contracts/reports/scaling-audit/findings-latest.json:21862`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0918 — `contracts/reports/scaling-audit/findings-latest.json:25504`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0919 — `contracts/reports/scaling-audit/findings-latest.json:25516`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0920 — `contracts/reports/scaling-audit/findings-latest.json:25538`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0921 — `contracts/reports/scaling-audit/findings-latest.json:25550`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0922 — `contracts/reports/scaling-audit/findings-latest.json:25868`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0923 — `contracts/reports/scaling-audit/findings-latest.json:26521`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0924 — `contracts/reports/scaling-audit/findings-latest.json:26533`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0925 — `contracts/reports/scaling-audit/findings-latest.json:26544`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0926 — `contracts/reports/scaling-audit/findings-latest.json:26556`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0927 — `contracts/reports/scaling-audit/findings-latest.json:27541`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0928 — `contracts/reports/scaling-audit/findings-latest.json:28607`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0929 — `contracts/reports/scaling-audit/findings-latest.json:29425`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0930 — `contracts/reports/scaling-audit/findings-latest.json:33724`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0931 — `contracts/reports/scaling-audit/findings-latest.json:33770`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0932 — `contracts/reports/scaling-audit/findings-latest.json:37507`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0933 — `contracts/reports/scaling-audit/findings-latest.json:37519`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0934 — `contracts/reports/scaling-audit/findings-latest.json:39787`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0935 — `contracts/reports/scaling-audit/findings-latest.json:40308`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0936 — `contracts/reports/scaling-audit/findings-latest.json:40320`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0937 — `contracts/reports/scaling-audit/findings-latest.json:40499`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0938 — `contracts/reports/scaling-audit/findings-latest.json:40511`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0939 — `contracts/reports/scaling-audit/findings-latest.json:40523`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0940 — `contracts/reports/scaling-audit/findings-latest.json:40547`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0941 — `contracts/reports/scaling-audit/findings-latest.json:40559`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0942 — `contracts/reports/scaling-audit/findings-latest.json:40571`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0943 — `contracts/reports/scaling-audit/findings-latest.json:40583`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0944 — `contracts/reports/scaling-audit/findings-latest.json:40595`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0945 — `contracts/reports/scaling-audit/findings-latest.json:40607`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0946 — `contracts/reports/scaling-audit/findings-latest.json:40619`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0947 — `contracts/reports/scaling-audit/findings-latest.json:40631`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0948 — `contracts/reports/scaling-audit/findings-latest.json:40643`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0949 — `contracts/reports/scaling-audit/findings-latest.json:40655`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0950 — `contracts/reports/scaling-audit/findings-latest.json:40667`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0951 — `contracts/reports/scaling-audit/findings-latest.json:40679`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0952 — `contracts/reports/scaling-audit/findings-latest.json:40691`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0953 — `contracts/reports/scaling-audit/findings-latest.json:40703`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0954 — `contracts/reports/scaling-audit/findings-latest.json:40715`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0955 — `contracts/reports/scaling-audit/findings-latest.json:40727`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0956 — `contracts/reports/scaling-audit/findings-latest.json:40768`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0957 — `contracts/reports/scaling-audit/findings-latest.json:40780`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0958 — `contracts/reports/scaling-audit/findings-latest.json:40792`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0959 — `contracts/reports/scaling-audit/findings-latest.json:40872`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0960 — `contracts/reports/scaling-audit/findings-latest.json:40884`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0961 — `contracts/reports/scaling-audit/findings-latest.json:40896`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0962 — `contracts/reports/scaling-audit/findings-latest.json:40908`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0963 — `contracts/reports/scaling-audit/findings-latest.json:40920`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0964 — `contracts/reports/scaling-audit/findings-latest.json:40932`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0965 — `contracts/reports/scaling-audit/findings-latest.json:40944`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0966 — `contracts/reports/scaling-audit/findings-latest.json:40956`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0967 — `contracts/reports/scaling-audit/findings-latest.json:40968`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0968 — `contracts/reports/scaling-audit/findings-latest.json:40980`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0969 — `contracts/reports/scaling-audit/findings-latest.json:40992`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-0970 — `contracts/reports/scaling-audit/findings-latest.json:41015`

**Substring**

```text
Regex::new(
```

**Why it matters**: Regexes without Unicode awareness or lazy quantifiers mis-parse real text.

**Fix** (fix-regex-unicode-or-quantifiers): Add (?u), prefer \p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.

**Verify**: `rg -nF "Regex::new(" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

