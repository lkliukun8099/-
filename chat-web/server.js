/**
 * 麦兜错题助手 - 对话式错题分析服务
 * 单端口 HTTP 应用：静态页面 + /api/analyze
 * 双引擎：BYOK（用户自带 OpenAI 兼容 Key，服务端代理）+ 无 Key 本地规则引擎
 */
const express = require("express");
const path = require("path");

const app = express();
const PORT = process.env.PORT || 3000;

app.use(express.json({ limit: "1mb" }));
app.use(express.static(path.join(__dirname, "public")));

/* ================= 本地规则引擎 ================= */

/** 从题干提取数字（用于无 Key 时的变式题参数扰动） */
function extractNumbers(text) {
  const m = String(text).match(/-?\d+(?:\.\d+)?/g);
  return m ? m.map(Number).filter((n) => Number.isFinite(n)) : [];
}

/** 通用变式：同题换数字 */
function numericVariants(question, count = 3) {
  const variants = [];
  const seeds = [1, 2, 3];
  for (let i = 0; i < count; i++) {
    let k = 0;
    const q = question.replace(/-?\d+(?:\.\d+)?/g, (s) => {
      k += 1;
      const n = Number(s);
      if (!Number.isFinite(n) || k > 6) return s;
      const delta = seeds[i % seeds.length] + k;
      const v = Math.abs(n) >= 10 ? n + delta : n + (i + 1);
      return String(v);
    });
    variants.push({
      question: q,
      hint: "结构与原题完全一致，仅数值变化，用同一方法求解。",
      answer: "（把原题解法套用到新数值即可；如需详解可配置大模型 Key 后重试。）",
    });
  }
  return variants;
}

/** 学科识别 */
function detectSubject(q) {
  const rules = [
    [/二次函数|抛物线|单调|判别式|定义域|值域|三角函数|诱导公式|数列|等差|等比|导数|切线|集合|不等式|函数|概率|向量|椭圆|双曲线/, "math"],
    [/受力|摩擦力|牛顿|加速度|匀变速|自由落体|动能|势能|电路|电压|电流|磁场|楞次|动量|机械能|传送带|斜面/, "physics"],
    [/化学方程式|配平|物质的量|摩尔|氧化还原|离子方程式|电解|水解|酸碱/, "chemistry"],
    [/时态|定语从句|非谓语|虚拟语气|完形|阅读|语法|单词|英语|过去式|现在完成/, "english"],
  ];
  for (const [re, s] of rules) if (re.test(q)) return s;
  return "general";
}

/** 错因规则库：命中→错误点+指正+变式题生成器 */
const MATH_RULES = [
  {
    id: "quadratic-monotonic",
    name: "二次函数对称轴与区间单调性",
    match: (q) => /二次函数|f\(x\)\s*=\s*x\s*[²2]/.test(q) && /单调|递增|递减|对称轴/.test(q),
    errorPoint: "二次函数对称轴与区间位置关系理解偏差",
    correction:
      "f(x)=x²-2ax+3 开口向上，对称轴 x=a。在区间上单调递减 ⟺ 区间整体位于对称轴左侧，即 a ≥ 区间右端点；单调递增 ⟺ a ≤ 区间左端点。易错点：把端点等号漏掉（端点处单调性仍成立，取闭区间比较），或把「递减⇒a≥2」误记成「a≤2」。",
    variants: (q) => {
      const vs = [];
      if (/递减/.test(q)) {
        vs.push({
          question: q.replace(/递减/g, "递增"),
          hint: "把条件从递减改为递增，比较方向随之翻转。",
          answer: "单调递增 ⟺ 对称轴 x=a ≤ 区间左端点 1，即 a ≤ 1，取值范围 (-∞, 1]。",
        });
      } else {
        vs.push({
          question: q.replace(/递增/g, "递减"),
          hint: "把条件从递增改为递减，比较方向随之翻转。",
          answer: "单调递减 ⟺ 对称轴 x=a ≥ 区间右端点 2，即 a ≥ 2，取值范围 [2, +∞)。",
        });
      }
      vs.push({
        question: "已知函数 f(x)=x²-2ax+3，若 f(x) 在区间 [1,2] 上不单调，求 a 的取值范围。",
        hint: "不单调 ⟺ 对称轴落在区间内部。",
        answer: "对称轴 x=a ∈ (1,2)，即 1 < a < 2（端点取不到，否则在端点处仍单调）。",
      });
      vs.push({
        question: "已知函数 f(x)=x²-2ax+3 在区间 [a-1, a+1] 上有最小值 2，求 a 的值。",
        hint: "对称轴必在该区间内部，最小值在顶点处取得。",
        answer: "最小值 f(a)=3-a²=2，解得 a=±1。",
      });
      return vs;
    },
  },
  {
    id: "discriminant",
    name: "一元二次方程判别式",
    match: (q) => /判别式|Δ|△|根的个数|两个不相等|有两个实数根|无实数根/.test(q),
    errorPoint: "判别式 Δ 与根的个数关系记忆/应用错误",
    correction:
      "Δ=b²-4ac：Δ>0 两个不相等实根；Δ=0 两个相等实根（重根）；Δ<0 无实根。高频错误：①题目含「有实根」时漏写 Δ≥0 的等号；②二次项系数含参数时忘记讨论系数为 0（退化为一次方程）的情形。",
    variants: () => [
      {
        question: "关于 x 的方程 x²-2x+m=0 有两个不相等的实数根，求 m 的取值范围。",
        hint: "Δ>0，注意严格不等号。",
        answer: "Δ=4-4m>0 ⟹ m<1，即 m∈(-∞,1)。",
      },
      {
        question: "关于 x 的方程 (m-1)x²+2x+1=0 有实数根，求 m 的取值范围。",
        hint: "陷阱：m-1=0 时退化为一次方程，也有实根，必须单独讨论。",
        answer: "① m=1 时 2x+1=0 有实根；② m≠1 时 Δ=4-4(m-1)≥0 ⟹ m≤2。综上 m≤2。",
      },
      {
        question: "若方程 x²+2(m-1)x+m²=0 无实数根，求 m 的取值范围。",
        hint: "Δ<0。",
        answer: "Δ=4(m-1)²-4m²=-8m+4<0 ⟹ m>1/2。",
      },
    ],
  },
  {
    id: "domain",
    name: "函数定义域",
    match: (q) => /定义域/.test(q),
    errorPoint: "定义域限制条件遗漏（分母/偶次根号/对数真数）",
    correction:
      "求定义域要同时检查三类限制：分母≠0、偶次根号内≥0、对数真数>0，最后取交集。易错：多个条件只取其一，或根号内写成 >0（应为 ≥0）。",
    variants: () => [
      {
        question: "求函数 y=√(x+2)/(x-1) 的定义域。",
        hint: "根号内 ≥0 且分母 ≠0，取交集。",
        answer: "x+2≥0 且 x-1≠0 ⟹ x≥-2 且 x≠1，定义域 [-2,1)∪(1,+∞)。",
      },
      {
        question: "求函数 y=lg(3-x)+√x 的定义域。",
        hint: "对数真数 >0，根号内 ≥0。",
        answer: "3-x>0 且 x≥0 ⟹ 0≤x<3，定义域 [0,3)。",
      },
      {
        question: "求函数 y=√(x²-3x+2) 的定义域。",
        hint: "解一元二次不等式，注意取两边还是中间。",
        answer: "x²-3x+2≥0 ⟹ (x-1)(x-2)≥0 ⟹ x≤1 或 x≥2，定义域 (-∞,1]∪[2,+∞)。",
      },
    ],
  },
  {
    id: "sequence",
    name: "等差/等比数列",
    match: (q) => /等差数列|等比数列|前n项和|Sn|S_n/.test(q),
    errorPoint: "数列通项与求和公式应用错误（公差/公比符号、项数错位）",
    correction:
      "等差：an=a₁+(n-1)d，Sn=n(a₁+an)/2=na₁+n(n-1)d/2；等比：an=a₁qⁿ⁻¹，q≠1 时 Sn=a₁(1-qⁿ)/(1-q)。易错：①n 从 0 还是 1 开始错位；②等比 q=1 忘记单独讨论；③Sn-Sn₋₁=an 在 n=1 时不成立。",
    variants: () => [
      {
        question: "等差数列 {an} 中 a₁=2，d=3，求 a₁₀ 与 S₁₀。",
        hint: "直接套通项与求和公式。",
        answer: "a₁₀=2+9×3=29；S₁₀=10×2+10×9×3/2=155。",
      },
      {
        question: "等比数列 {an} 中 a₁=1，q=2，求 S₅。",
        hint: "q≠1，用 Sn=a₁(1-qⁿ)/(1-q)。",
        answer: "S₅=(1-2⁵)/(1-2)=31。",
      },
      {
        question: "已知数列前 n 项和 Sn=n²+2n，求通项 an。",
        hint: "an=Sn-Sn₋₁（n≥2），a₁ 单独验证。",
        answer: "n≥2 时 an=2n+1；a₁=S₁=3 也满足，故 an=2n+1。",
      },
    ],
  },
  {
    id: "derivative",
    name: "导数与切线",
    match: (q) => /导数|切线|极值|单调区间|f'/.test(q),
    errorPoint: "导数几何意义或求导法则应用错误",
    correction:
      "切线斜率 k=f'(x₀)，切线方程 y-f(x₀)=f'(x₀)(x-x₀)。高频错误：①「在点处」与「过点」混淆——过点 P 的切线，P 不一定是切点，需设切点 (t,f(t)) 列方程；②乘积/复合函数求导法则用错。",
    variants: () => [
      {
        question: "求曲线 y=x³-3x 在点 (1,-2) 处的切线方程。",
        hint: "先验证该点在曲线上，再求 k=f'(1)。",
        answer: "y'=3x²-3，k=0，切线 y=-2。",
      },
      {
        question: "求曲线 y=x² 过点 (0,-1) 的切线方程。",
        hint: "陷阱：(0,-1) 不在曲线上！设切点 (t,t²)。",
        answer: "切线 y-t²=2t(x-t) 过 (0,-1) ⟹ -t²=-1 ⟹ t=±1，切线为 y=2x-1 或 y=-2x-1。",
      },
      {
        question: "求 f(x)=x³-3x²+1 的单调递减区间。",
        hint: "解 f'(x)<0。",
        answer: "f'(x)=3x²-6x=3x(x-2)<0 ⟹ 0<x<2，减区间 (0,2)。",
      },
    ],
  },
];

const PHYSICS_RULES = [
  {
    id: "friction",
    name: "摩擦力方向判断",
    match: (q) => /摩擦力|受力分析|传送带|斜面/.test(q),
    errorPoint: "摩擦力方向判断错误（相对运动趋势分析不清）",
    correction:
      "摩擦力方向与「相对运动（趋势）」方向相反，而不是与运动方向相反。判断步骤：①选研究对象；②假设接触面光滑，判断相对运动趋势方向；③摩擦力与之相反。传送带模型中，物体速度小于带速时摩擦力为动力（与运动同向），这是最高频的错误点。",
    variants: () => [
      {
        question: "物块轻放在水平向右匀速运动的传送带上，刚放上去瞬间，物块受到的摩擦力方向如何？",
        hint: "物块速度为 0，带速向右，判断相对运动趋势。",
        answer: "物块相对传送带有向左的运动趋势，故摩擦力向右（动力，使物块加速）。",
      },
      {
        question: "物块静止在粗糙斜面上，受到几个力？摩擦力方向沿斜面向哪？",
        hint: "重力分解后，下滑趋势沿斜面向下。",
        answer: "受重力、支持力、静摩擦力三个力；摩擦力沿斜面向上，与下滑趋势相反。",
      },
      {
        question: "人用水平力 F 推静止在水平地面上的箱子但没推动，此时箱子受摩擦力吗？大小方向如何？",
        hint: "静止 ⟹ 合力为零。",
        answer: "受静摩擦力，大小等于 F，方向与 F 相反（静摩擦力随外力自适应，不超过最大静摩擦）。",
      },
    ],
  },
  {
    id: "newton2",
    name: "牛顿第二定律",
    match: (q) => /加速度|牛顿第二|合力|F=ma|超重|失重/.test(q),
    errorPoint: "牛顿第二定律矢量性/瞬时性理解偏差",
    correction:
      "F合=ma：a 与合力同向且瞬时对应。易错：①用某个分力代替合力；②超重失重只看加速度方向（a 向上→超重，a 向下→失重），与速度方向无关。",
    variants: () => [
      {
        question: "质量 2kg 的物体受水平拉力 10N，地面摩擦力 4N，求加速度。",
        hint: "用合力而非拉力。",
        answer: "F合=10-4=6N，a=6/2=3 m/s²。",
      },
      {
        question: "电梯加速上升时，人对电梯地板的压力与重力相比如何？",
        hint: "加速度向上 ⟹ 支持力 > 重力。",
        answer: "N-mg=ma>0 ⟹ N>mg，压力大于重力（超重）。",
      },
      {
        question: "物体以初速度 v₀ 竖直上抛，到达最高点瞬间加速度是多少？",
        hint: "陷阱：速度为零但加速度不为零。",
        answer: "a=g，方向竖直向下（只受重力，与速度无关）。",
      },
    ],
  },
];

const CHEM_RULES = [
  {
    id: "balance",
    name: "化学方程式配平",
    match: (q) => /配平|化学方程式|离子方程式/.test(q),
    errorPoint: "方程式配平或电荷守恒应用错误",
    correction:
      "配平顺序：先原子守恒（先配 H、O 以外的元素），再电荷守恒（离子方程式），最后检查。氧化还原用「升降价相等」定系数。易错：改变化学式下标（绝对禁止），只能改化学计量数。",
    variants: () => [
      {
        question: "配平：__Fe + __O₂ → __Fe₃O₄",
        hint: "先配 Fe，再配 O。",
        answer: "3Fe + 2O₂ = Fe₃O₄。",
      },
      {
        question: "配平离子方程式：__Fe²⁺ + __H⁺ + __NO₃⁻ → __Fe³⁺ + __NO↑ + __H₂O",
        hint: "Fe 升 1 价，N 降 3 价，按 3:1 定 Fe²⁺ 与 NO₃⁻ 系数。",
        answer: "3Fe²⁺ + 4H⁺ + NO₃⁻ = 3Fe³⁺ + NO↑ + 2H₂O。",
      },
      {
        question: "下列写法正确吗？把 H₂O 的系数配成 2 后，为配平氢原子把 H₂ 改成 H₃。",
        hint: "下标是物质的组成，不能动。",
        answer: "错误。只能调整化学式前面的计量数，绝不能改动下标（H₂ 是氢气分子的固定组成）。",
      },
    ],
  },
];

const EN_RULES = [
  {
    id: "tense",
    name: "动词时态",
    match: (q) => /时态|过去式|现在完成|过去完成|since|for \+|already|yet/i.test(q),
    errorPoint: "时态选择错误（时间标志词识别与动作先后关系不清）",
    correction:
      "先找时间标志词：since/for+时间段/already/yet → 现在完成时；by+过去时间/before+过去动作 → 过去完成时（过去的过去）；yesterday/last week → 一般过去时。易错：for+时间段误配一般过去时。",
    variants: () => [
      {
        question: "I _____ (live) in Nanjing since 2015. 用适当形式填空。",
        hint: "since+过去时间点 → 现在完成时。",
        answer: "have lived（从过去持续到现在的状态）。",
      },
      {
        question: "By the end of last year, we _____ (learn) 2000 words.",
        hint: "by+过去时间 → 过去的过去。",
        answer: "had learned（过去完成时）。",
      },
      {
        question: "He _____ (go) to Beijing yesterday. A. has gone B. went C. goes",
        hint: "yesterday 是确定的过去时间，不用完成时。",
        answer: "B. went（一般过去时；has gone 与具体过去时间状语不连用）。",
      },
    ],
  },
];

const RULESETS = { math: MATH_RULES, physics: PHYSICS_RULES, chemistry: CHEM_RULES, english: EN_RULES };

/** 本地引擎分析 */
function localAnalyze(question, myAnswer) {
  const subject = detectSubject(question);
  const rules = RULESETS[subject] || [];
  const hit = rules.find((r) => r.match(question));
  if (hit) {
    return {
      engine: "local",
      subject,
      errorPoints: [hit.errorPoint, ...(myAnswer ? ["你的作答与标准解法的关键步骤存在偏差（详见指正）"] : [])],
      correction: hit.correction,
      variants: hit.variants(question).slice(0, 3),
    };
  }
  // 未命中规则：通用兜底（换数字变式）
  const hasNums = extractNumbers(question).length > 0;
  return {
    engine: "local",
    subject,
    errorPoints: [
      "本地规则库未命中精确错因（建议配置大模型 Key 获得深度归因）",
      "请先自查：审题是否漏条件、公式/定理是否用错前提、计算过程是否有符号错误",
    ],
    correction:
      "通用订正流程：①把题干条件逐条翻译成数学/物理语言；②定位本题考查的核心知识点；③写出标准解法的每一步依据；④与你的作答逐行对比，第一个分岔点就是错误根源。",
    variants: hasNums
      ? numericVariants(question, 3)
      : [
          { question: `【变式1】${question}（请将原题中的关键条件或数值自行替换后重做一遍）`, hint: "主动改条件是最好的巩固。", answer: "—" },
          { question: "【变式2】请用自己的话向同学讲清本题的完整解法，并指出最容易错的一步。", hint: "费曼学习法：讲不出来就是没真懂。", answer: "—" },
          { question: "【变式3】找一道教材/练习册中与本题同知识点的题目完成，并对比两题条件的异同。", hint: "同构异景是迁移能力的关键。", answer: "—" },
        ],
  };
}

/* ================= LLM（BYOK 代理） ================= */

const SYS_PROMPT = `你是「麦兜」错题分析助手。用户会给你一道他做错的题目（可能附他的错误作答）。
你必须**只输出一个 JSON 对象**（不要输出任何其他文字、不要用 markdown 代码围栏），结构如下：
{
  "subject": "math|physics|chemistry|english|biology|general",
  "errorPoints": ["错误点1（具体到知识点与思维误区，不要用泛泛的'粗心')", "错误点2"],
  "correction": "指正与正确解法：先指出错误根源，再给完整正确解题过程（可用 LaTeX 行内公式 $...$）",
  "variants": [
    {"question": "变式题1（同知识点、难度相当或略低）", "hint": "提示", "answer": "答案与简解"},
    {"question": "变式题2（同构异景，换一个情境）", "hint": "提示", "answer": "答案与简解"},
    {"question": "变式题3（递进难度，综合一点）", "hint": "提示", "answer": "答案与简解"}
  ]
}
要求：变式题必须与错题同知识点且**答案经过你自己验算**，不得编造错题；题目表述完整可独立作答。`;

async function llmAnalyze(question, myAnswer, llm) {
  const baseUrl = (llm.baseUrl || "https://api.deepseek.com").replace(/\/+$/, "");
  const model = llm.model || "deepseek-chat";
  const userContent = myAnswer ? `【题目】${question}\n【我的（错误）作答】${myAnswer}` : `【题目】${question}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 60000);
  try {
    const resp = await fetch(`${baseUrl}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${llm.apiKey}`,
      },
      body: JSON.stringify({
        model,
        temperature: 0.3,
        messages: [
          { role: "system", content: SYS_PROMPT },
          { role: "user", content: userContent },
        ],
      }),
      signal: controller.signal,
    });
    if (!resp.ok) {
      const t = await resp.text().catch(() => "");
      const err = new Error(`LLM API ${resp.status}: ${t.slice(0, 200)}`);
      err.status = resp.status;
      throw err;
    }
    const data = await resp.json();
    const text = data.choices?.[0]?.message?.content || "";
    // 容错解析：去代码围栏、截取首个 { 到末个 }
    const cleaned = text.replace(/```(?:json)?/gi, "");
    const s = cleaned.indexOf("{");
    const e = cleaned.lastIndexOf("}");
    if (s === -1 || e === -1) throw new Error("模型输出非 JSON，已回退展示原文");
    const parsed = JSON.parse(cleaned.slice(s, e + 1));
    return {
      engine: "llm",
      model,
      subject: parsed.subject || "general",
      errorPoints: Array.isArray(parsed.errorPoints) ? parsed.errorPoints : [String(parsed.errorPoints || "解析失败")],
      correction: String(parsed.correction || ""),
      variants: (Array.isArray(parsed.variants) ? parsed.variants : []).slice(0, 3),
      raw: text,
    };
  } finally {
    clearTimeout(timer);
  }
}

/* ================= 路由 ================= */

app.post("/api/analyze", async (req, res) => {
  const { question, myAnswer, llm } = req.body || {};
  if (!question || typeof question !== "string" || question.trim().length < 4) {
    return res.status(400).json({ error: "请把题目写完整（至少 4 个字符）" });
  }
  try {
    if (llm && llm.apiKey) {
      try {
        const r = await llmAnalyze(question.trim(), (myAnswer || "").trim(), llm);
        return res.json(r);
      } catch (e) {
        // LLM 失败自动降级本地引擎，并告知原因
        const r = localAnalyze(question.trim(), (myAnswer || "").trim());
        r.fallbackReason = e.message;
        return res.json(r);
      }
    }
    return res.json(localAnalyze(question.trim(), (myAnswer || "").trim()));
  } catch (e) {
    return res.status(500).json({ error: "分析失败：" + e.message });
  }
});

app.get("/api/health", (_req, res) => res.json({ ok: true, name: "maidoupig-chat" }));

app.listen(PORT, "0.0.0.0", () => {
  console.log(`[maidoupig-chat] listening on 0.0.0.0:${PORT}`);
});
