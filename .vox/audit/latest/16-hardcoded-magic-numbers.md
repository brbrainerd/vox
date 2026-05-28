# 16 — hardcoded magic numbers

**Severity**: info  
**Itemized**: 100

### hv-1094 — `apps/editor/vox-vscode/src/commands/commandCatalog.ts:98`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "apps/editor/vox-vscode/src/commands/commandCatalog.ts"`

**Confidence**: medium

---

### hv-1095 — `contracts/db/data-storage-policy.v1.yaml:60`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/db/data-storage-policy.v1.yaml"`

**Confidence**: low

---

### hv-1096 — `contracts/dei/rpc-methods.schema.json:198`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/dei/rpc-methods.schema.json"`

**Confidence**: low

---

### hv-1097 — `contracts/orchestration/journey-envelope.v1.schema.json:18`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/orchestration/journey-envelope.v1.schema.json"`

**Confidence**: low

---

### hv-1098 — `contracts/orchestration/journey-envelope.v1.schema.json:23`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/orchestration/journey-envelope.v1.schema.json"`

**Confidence**: low

---

### hv-1099 — `contracts/orchestration/journey-envelope.v1.schema.json:28`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/orchestration/journey-envelope.v1.schema.json"`

**Confidence**: low

---

### hv-1100 — `contracts/populi/control-plane.openapi.yaml:551`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/populi/control-plane.openapi.yaml"`

**Confidence**: low

---

### hv-1101 — `contracts/reports/scaling-audit/findings-latest.json:477`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1102 — `contracts/reports/scaling-audit/findings-latest.json:17130`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1103 — `contracts/reports/scaling-audit/findings-latest.json:17859`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1104 — `contracts/reports/scaling-audit/findings-latest.json:18230`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1105 — `contracts/reports/scaling-audit/findings-latest.json:18865`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1106 — `contracts/reports/scaling-audit/findings-latest.json:18888`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1107 — `contracts/reports/scaling-audit/findings-latest.json:21092`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1108 — `contracts/reports/scaling-audit/findings-latest.json:21104`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1109 — `contracts/reports/scaling-audit/findings-latest.json:21398`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1110 — `contracts/reports/scaling-audit/findings-latest.json:26685`

**Substring**

```text
16384
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "16384" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1111 — `contracts/reports/scaling-audit/findings-latest.json:26819`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1112 — `contracts/reports/scaling-audit/findings-latest.json:26912`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1113 — `contracts/reports/scaling-audit/findings-latest.json:26924`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1114 — `contracts/reports/scaling-audit/findings-latest.json:26936`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1115 — `contracts/reports/scaling-audit/findings-latest.json:26948`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1116 — `contracts/reports/scaling-audit/findings-latest.json:26960`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1117 — `contracts/reports/scaling-audit/findings-latest.json:27032`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1118 — `contracts/reports/scaling-audit/findings-latest.json:27074`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1119 — `contracts/reports/scaling-audit/findings-latest.json:27250`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1120 — `contracts/reports/scaling-audit/findings-latest.json:27398`

**Substring**

```text
65536
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "65536" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1121 — `contracts/reports/scaling-audit/findings-latest.json:27455`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1122 — `contracts/reports/scaling-audit/findings-latest.json:27529`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1123 — `contracts/reports/scaling-audit/findings-latest.json:28057`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1124 — `contracts/reports/scaling-audit/findings-latest.json:28069`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1125 — `contracts/reports/scaling-audit/findings-latest.json:28092`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1126 — `contracts/reports/scaling-audit/findings-latest.json:28104`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1127 — `contracts/reports/scaling-audit/findings-latest.json:28887`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1128 — `contracts/reports/scaling-audit/findings-latest.json:30425`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1129 — `contracts/reports/scaling-audit/findings-latest.json:30722`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1130 — `contracts/reports/scaling-audit/findings-latest.json:30734`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1131 — `contracts/reports/scaling-audit/findings-latest.json:31010`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1132 — `contracts/reports/scaling-audit/findings-latest.json:31022`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1133 — `contracts/reports/scaling-audit/findings-latest.json:31034`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1134 — `contracts/reports/scaling-audit/findings-latest.json:31058`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1135 — `contracts/reports/scaling-audit/findings-latest.json:31082`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1136 — `contracts/reports/scaling-audit/findings-latest.json:31094`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1137 — `contracts/reports/scaling-audit/findings-latest.json:31153`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1138 — `contracts/reports/scaling-audit/findings-latest.json:31183`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1139 — `contracts/reports/scaling-audit/findings-latest.json:31206`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1140 — `contracts/reports/scaling-audit/findings-latest.json:32097`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1141 — `contracts/reports/scaling-audit/findings-latest.json:32481`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1142 — `contracts/reports/scaling-audit/findings-latest.json:32493`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1143 — `contracts/reports/scaling-audit/findings-latest.json:32505`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1144 — `contracts/reports/scaling-audit/findings-latest.json:32517`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1145 — `contracts/reports/scaling-audit/findings-latest.json:32529`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1146 — `contracts/reports/scaling-audit/findings-latest.json:32560`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: high

---

### hv-1147 — `contracts/reports/scaling-audit/findings-latest.json:32650`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1148 — `contracts/reports/scaling-audit/findings-latest.json:32662`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1149 — `contracts/reports/scaling-audit/findings-latest.json:32674`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1150 — `contracts/reports/scaling-audit/findings-latest.json:32686`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1151 — `contracts/reports/scaling-audit/findings-latest.json:32698`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1152 — `contracts/reports/scaling-audit/findings-latest.json:32710`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1153 — `contracts/reports/scaling-audit/findings-latest.json:32722`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1154 — `contracts/reports/scaling-audit/findings-latest.json:32758`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1155 — `contracts/reports/scaling-audit/findings-latest.json:33394`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1156 — `contracts/reports/scaling-audit/findings-latest.json:33406`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1157 — `contracts/reports/scaling-audit/findings-latest.json:33567`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1158 — `contracts/reports/scaling-audit/findings-latest.json:34003`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1159 — `contracts/reports/scaling-audit/findings-latest.json:34069`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1160 — `contracts/reports/scaling-audit/findings-latest.json:34081`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1161 — `contracts/reports/scaling-audit/findings-latest.json:34948`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1162 — `contracts/reports/scaling-audit/findings-latest.json:35325`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1163 — `contracts/reports/scaling-audit/findings-latest.json:35944`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1164 — `contracts/reports/scaling-audit/findings-latest.json:35956`

**Substring**

```text
16384
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "16384" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1165 — `contracts/reports/scaling-audit/findings-latest.json:36012`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1166 — `contracts/reports/scaling-audit/findings-latest.json:36089`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1167 — `contracts/reports/scaling-audit/findings-latest.json:36101`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1168 — `contracts/reports/scaling-audit/findings-latest.json:36352`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1169 — `contracts/reports/scaling-audit/findings-latest.json:36364`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1170 — `contracts/reports/scaling-audit/findings-latest.json:36376`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1171 — `contracts/reports/scaling-audit/findings-latest.json:36489`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1172 — `contracts/reports/scaling-audit/findings-latest.json:36839`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1173 — `contracts/reports/scaling-audit/findings-latest.json:36875`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1174 — `contracts/reports/scaling-audit/findings-latest.json:36887`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1175 — `contracts/reports/scaling-audit/findings-latest.json:38579`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1176 — `contracts/reports/scaling-audit/findings-latest.json:40209`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1177 — `contracts/reports/scaling-audit/findings-latest.json:40243`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1178 — `contracts/reports/scaling-audit/findings-latest.json:40255`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1179 — `contracts/reports/scaling-audit/findings-latest.json:40297`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1180 — `contracts/reports/scaling-audit/findings-latest.json:40344`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: medium

---

### hv-1181 — `contracts/reports/scaling-audit/findings-latest.json:40476`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1182 — `contracts/reports/scaling-audit/findings-latest.json:40488`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1183 — `contracts/reports/scaling-audit/findings-latest.json:41370`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1184 — `contracts/reports/scaling-audit/findings-latest.json:41382`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1185 — `contracts/reports/scaling-audit/findings-latest.json:41394`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1186 — `contracts/reports/scaling-audit/findings-latest.json:41505`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1187 — `contracts/reports/scaling-audit/findings-latest.json:41517`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-latest.json"`

**Confidence**: low

---

### hv-1188 — `contracts/reports/scaling-audit/findings-scaling-latest.json:99`

**Substring**

```text
2048
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "2048" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

### hv-1189 — `contracts/reports/scaling-audit/findings-scaling-latest.json:385`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

### hv-1190 — `contracts/reports/scaling-audit/findings-scaling-latest.json:1936`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

### hv-1191 — `contracts/reports/scaling-audit/findings-scaling-latest.json:1947`

**Substring**

```text
4096
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "4096" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

### hv-1192 — `contracts/reports/scaling-audit/findings-scaling-latest.json:1958`

**Substring**

```text
8192
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "8192" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

### hv-1193 — `contracts/reports/scaling-audit/findings-scaling-latest.json:2266`

**Substring**

```text
1024
```

**Why it matters**: Unnamed numeric literals lack units and intent; reviewers cannot tune safely.

**Fix** (extract-named-constant): const MEANINGFUL_NAME: u64 = …; // explain units

**Verify**: `rg -nF "1024" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: low

---

