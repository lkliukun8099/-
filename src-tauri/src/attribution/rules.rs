//! 归因规则库（首批数学核心规则，Q-8：每学科 50-100 条目标，可扩充）。
//!
//! 规则结构：题干关键词（命中加权）+ 作答特征（正则，可选）
//! → 错因标签 + 可解释说明 + 图谱三级节点编码。
//! 匹配策略：题干命中数 ×1 + 作答命中数 ×2 加权，取最高分规则。

use regex::Regex;

/// 静态规则定义。
pub struct AttributionRule {
    /// 题干关键词（任一命中 +1）
    pub stem_keywords: &'static [&'static str],
    /// 学生作答正则（任一命中 +2）
    pub answer_patterns: &'static [&'static str],
    pub cause_label: &'static str,
    pub cause_detail: &'static str,
    /// 图谱三级节点编码（苏教版数学种子）
    pub kg_code: &'static str,
    pub confidence: f64,
}

/// 规则命中结果。
#[derive(Debug, Clone)]
pub struct RuleHit {
    pub cause_label: String,
    pub cause_detail: String,
    pub kg_code: String,
    pub confidence: f64,
}

/// 编译后的规则（正则预编译）。
struct CompiledRule {
    stem_keywords: Vec<String>,
    answer_patterns: Vec<Regex>,
    cause_label: String,
    cause_detail: String,
    kg_code: String,
    confidence: f64,
}

/// 规则引擎。
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    pub fn new() -> Self {
        let rules = math_rules()
            .into_iter()
            .map(|r| CompiledRule {
                stem_keywords: r.stem_keywords.iter().map(|s| s.to_string()).collect(),
                answer_patterns: r
                    .answer_patterns
                    .iter()
                    .filter_map(|p| Regex::new(p).ok())
                    .collect(),
                cause_label: r.cause_label.to_string(),
                cause_detail: r.cause_detail.to_string(),
                kg_code: r.kg_code.to_string(),
                confidence: r.confidence,
            })
            .collect();
        Self { rules }
    }

    /// 规则匹配：返回最高分命中（无命中返回 None）。
    pub fn evaluate(&self, ocr_text: &str, student_answer: &str) -> Option<RuleHit> {
        let stem = ocr_text.to_lowercase();
        let answer = student_answer.to_lowercase();
        let mut best: Option<(&CompiledRule, u32)> = None;
        for rule in &self.rules {
            let stem_hits = rule
                .stem_keywords
                .iter()
                .filter(|k| stem.contains(&k.to_lowercase()))
                .count() as u32;
            if stem_hits == 0 {
                continue;
            }
            let answer_hits = rule
                .answer_patterns
                .iter()
                .filter(|re| re.is_match(&answer))
                .count() as u32;
            let score = stem_hits + answer_hits * 2;
            if score > best.map(|(_, s)| s).unwrap_or(0) {
                best = Some((rule, score));
            }
        }
        best.map(|(rule, score)| {
            // 命中强度微调置信度（封顶 0.9，规则初判留余量给云端深判）
            let conf = (rule.confidence + 0.02 * score as f64).min(0.9);
            RuleHit {
                cause_label: rule.cause_label.clone(),
                cause_detail: rule.cause_detail.clone(),
                kg_code: rule.kg_code.clone(),
                confidence: conf,
            }
        })
    }

    /// 规则总数（自检/展示用）。
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// 数学核心归因规则库（首批 40 条占位骨架，按同结构可扩充至 50-100 条）。
fn math_rules() -> Vec<AttributionRule> {
    vec![
        AttributionRule {
            stem_keywords: &["单调递减", "单调递增", "单调性", "对称轴"],
            answer_patterns: &[r"a\s*[≥>]\s*1", r"a\s*[≤<]"],
            cause_label: "对称轴与区间位置关系理解偏差",
            cause_detail: "二次函数在区间上的单调性取决于对称轴与区间的相对位置，需将对称轴 x=a 与区间端点比较，而非直接代入端点值。",
            kg_code: "MATH.SX.FUNC.FUNC.QUAD.T03",
            confidence: 0.78,
        },
        AttributionRule {
            stem_keywords: &["顶点", "最值", "最大值", "最小值"],
            answer_patterns: &[],
            cause_label: "二次函数最值求解方法错误",
            cause_detail: "求二次函数最值应化为顶点式或利用对称轴，需注意开口方向与定义域限制。",
            kg_code: "MATH.SX.FUNC.FUNC.QUAD.T04",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["判别式", "根的情况", "两个不相等", "实数根", "Δ", "△"],
            answer_patterns: &[r"[△Δδ]\s*=\s*b\^?2", r"delta"],
            cause_label: "判别式使用条件记忆不清",
            cause_detail: "判别式 Δ=b²-4ac 与根的个数对应关系混淆：Δ>0 两不等实根，Δ=0 两相等实根，Δ<0 无实根。",
            kg_code: "MATH.SX.NUM.NUM.EQ.T08",
            confidence: 0.62,
        },
        AttributionRule {
            stem_keywords: &["韦达", "根与系数", "x1", "x2"],
            answer_patterns: &[],
            cause_label: "根与系数关系应用错误",
            cause_detail: "韦达定理：x1+x2=-b/a，x1·x2=c/a，常见错误为符号遗漏或分母 a 缺失。",
            kg_code: "MATH.SX.NUM.NUM.EQ.T09",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["因式分解", "十字相乘"],
            answer_patterns: &[],
            cause_label: "因式分解符号处理错误",
            cause_detail: "十字相乘或公式法分解时符号搭配错误，建议展开回代检验。",
            kg_code: "MATH.SX.NUM.NUM.POLY.T06",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["分式方程", "增根"],
            answer_patterns: &[],
            cause_label: "分式方程忽略增根检验",
            cause_detail: "分式方程去分母后必须检验：使最简公分母为零的解是增根，应舍去。",
            kg_code: "MATH.SX.NUM.NUM.EQ.T12",
            confidence: 0.65,
        },
        AttributionRule {
            stem_keywords: &["不等式", "解集", "整数解"],
            answer_patterns: &[r"[≥>]\s*\d+\s*$"],
            cause_label: "不等式变号规则遗漏",
            cause_detail: "不等式两边同乘（除）负数时不等号方向必须改变；端点取舍需结合数轴判断。",
            kg_code: "MATH.SX.NUM.NUM.EQ.T10",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["绝对值"],
            answer_patterns: &[],
            cause_label: "绝对值分类讨论不完整",
            cause_detail: "含绝对值的式子应按零点分段讨论，常见错误为只讨论一种情形。",
            kg_code: "MATH.SX.NUM.NUM.RAT.T02",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["科学记数法"],
            answer_patterns: &[r"\d+\s*[×x]\s*10\^?\-?\d+"],
            cause_label: "科学记数法指数确定错误",
            cause_detail: "科学记数法 a×10^n 中 1≤|a|<10，n 由小数点移动位数与方向决定。",
            kg_code: "MATH.SX.NUM.NUM.RAT.T05",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["平方根", "算术平方根"],
            answer_patterns: &[r"[±+\-]?\s*√"],
            cause_label: "平方根与算术平方根概念混淆",
            cause_detail: "正数的平方根有两个（互为相反数），算术平方根只取非负值。",
            kg_code: "MATH.SX.NUM.NUM.REAL.T01",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["二次根式"],
            answer_patterns: &[],
            cause_label: "二次根式化简不彻底或出错",
            cause_detail: "二次根式化简需提取完全平方因数；√a²=|a|，需注意字母取值范围。",
            kg_code: "MATH.SX.NUM.NUM.REAL.T03",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["一次函数", "待定系数"],
            answer_patterns: &[],
            cause_label: "待定系数法列方程错误",
            cause_detail: "将点坐标代入 y=kx+b 列方程组时，注意横纵坐标对应关系与符号。",
            kg_code: "MATH.SX.FUNC.FUNC.LINEAR.T04",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["反比例"],
            answer_patterns: &[],
            cause_label: "反比例函数 k 的几何意义应用错误",
            cause_detail: "反比例函数 y=k/x 中 |k| 等于图象上点与坐标轴围成矩形面积，注意象限符号。",
            kg_code: "MATH.SX.FUNC.FUNC.INV.T02",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["指数函数", "对数函数", "对数"],
            answer_patterns: &[r"log"],
            cause_label: "指对数运算法则混用",
            cause_detail: "对数运算法则 log(MN)=logM+logN 与指数法则易混淆，注意定义域限制。",
            kg_code: "MATH.SX.FUNC.FUNC.HIGH.T02",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["奇偶性", "奇函数", "偶函数"],
            answer_patterns: &[],
            cause_label: "函数奇偶性判定忽略定义域",
            cause_detail: "判定奇偶性前必须确认定义域关于原点对称，再用 f(-x) 与 f(x) 关系判断。",
            kg_code: "MATH.SX.FUNC.FUNC.HIGH.T05",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["零点", "二分法"],
            answer_patterns: &[],
            cause_label: "函数零点存在性理解偏差",
            cause_detail: "零点存在性定理要求函数在区间上连续且端点函数值异号；零点不是点而是横坐标值。",
            kg_code: "MATH.SX.FUNC.FUNC.HIGH.T06",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["正弦", "余弦", "正切", "三角函数"],
            answer_patterns: &[r"sin", r"cos", r"tan"],
            cause_label: "锐角三角函数定义记忆错误",
            cause_detail: "sin=对边/斜边，cos=邻边/斜边，tan=对边/邻边，常见邻边对边颠倒。",
            kg_code: "MATH.SX.TRIG.TRIG.ACUTE.T01",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["解直角三角形", "仰角", "俯角"],
            answer_patterns: &[],
            cause_label: "解直角三角形边角关系选择错误",
            cause_detail: "根据已知元素选择恰当的三角函数关系式，注意仰角俯角与水平线的关系。",
            kg_code: "MATH.SX.TRIG.TRIG.ACUTE.T03",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["诱导公式"],
            answer_patterns: &[],
            cause_label: "诱导公式符号判断错误",
            cause_detail: "诱导公式口诀\"奇变偶不变，符号看象限\"，符号判断需把 α 视为锐角。",
            kg_code: "MATH.SX.TRIG.TRIG.FUNC.T03",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["正弦定理", "余弦定理"],
            answer_patterns: &[],
            cause_label: "正余弦定理适用条件混淆",
            cause_detail: "已知两边一对角用正弦定理（注意多解讨论），已知两边夹角或三边用余弦定理。",
            kg_code: "MATH.SX.TRIG.TRIG.FUNC.T06",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["向量", "数量积"],
            answer_patterns: &[],
            cause_label: "向量数量积公式应用错误",
            cause_detail: "a·b=|a||b|cosθ，注意夹角是两向量共起点时的夹角，坐标运算对应分量相乘求和。",
            kg_code: "MATH.SX.TRIG.TRIG.VEC.T03",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["全等", "sss", "sas", "asa", "aas"],
            answer_patterns: &[],
            cause_label: "全等三角形判定条件误用",
            cause_detail: "SSA 不能判定全等；注意对应顶点顺序与隐含公共边/公共角条件。",
            kg_code: "MATH.SX.GEO.GEO.TRI.T02",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["相似"],
            answer_patterns: &[],
            cause_label: "相似三角形对应关系找错",
            cause_detail: "相似三角形对应边成比例，需先确定对应顶点；常见错误为对应边配对错误。",
            kg_code: "MATH.SX.GEO.GEO.TRI.T09",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["勾股"],
            answer_patterns: &[],
            cause_label: "勾股定理使用前提遗漏",
            cause_detail: "勾股定理仅适用于直角三角形，需先确认直角；斜边为最长边。",
            kg_code: "MATH.SX.GEO.GEO.TRI.T05",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["平行四边形"],
            answer_patterns: &[],
            cause_label: "平行四边形判定条件混淆",
            cause_detail: "一组对边相等另一组对边平行不能判定平行四边形，需\"一组对边平行且相等\"。",
            kg_code: "MATH.SX.GEO.GEO.QUAD.T01",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["垂径定理"],
            answer_patterns: &[],
            cause_label: "垂径定理条件理解不全",
            cause_detail: "垂直于弦的直径平分弦且平分弦所对的弧，注意\"直径\"条件不可缺。",
            kg_code: "MATH.SX.GEO.GEO.CIRCLE.T01",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["圆周角", "圆心角"],
            answer_patterns: &[],
            cause_label: "圆周角与圆心角倍数关系用错",
            cause_detail: "同弧所对圆周角等于圆心角的一半，注意区分弧的优弧劣弧。",
            kg_code: "MATH.SX.GEO.GEO.CIRCLE.T03",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["切线"],
            answer_patterns: &[],
            cause_label: "切线判定与性质混用",
            cause_detail: "判定切线需\"过半径外端且垂直于半径\"；切线长定理注意从圆外一点引两条切线。",
            kg_code: "MATH.SX.GEO.GEO.CIRCLE.T05",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["弧长", "扇形"],
            answer_patterns: &[],
            cause_label: "弧长扇形面积公式记忆错误",
            cause_detail: "弧长 l=nπr/180，扇形面积 S=nπr²/360=lr/2，注意角度制与弧度制区别。",
            kg_code: "MATH.SX.GEO.GEO.CIRCLE.T07",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["三视图"],
            answer_patterns: &[],
            cause_label: "三视图对应关系判断错误",
            cause_detail: "主视图反映长与高，俯视图反映长与宽，左视图反映宽与高，注意虚实线。",
            kg_code: "MATH.SX.GEO.GEO.SOLID.T01",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["椭圆", "双曲线", "抛物线", "焦点"],
            answer_patterns: &[],
            cause_label: "圆锥曲线定义中参数关系混淆",
            cause_detail: "椭圆 a²=b²+c² 而双曲线 c²=a²+b²，抛物线 p 的几何意义是焦点到准线距离。",
            kg_code: "MATH.SX.GEO.GEO.ANALYTIC.T07",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["斜率", "直线方程"],
            answer_patterns: &[],
            cause_label: "直线斜率不存在情形遗漏",
            cause_detail: "垂直于 x 轴的直线斜率不存在，设直线方程时需分类讨论或设 x=my+n 形式。",
            kg_code: "MATH.SX.GEO.GEO.ANALYTIC.T01",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["平均数", "中位数", "众数"],
            answer_patterns: &[],
            cause_label: "统计量概念混淆",
            cause_detail: "中位数需先排序取中间值（偶数个取平均），众数是出现次数最多的值。",
            kg_code: "MATH.SX.STAT.STAT.STAT.T01",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["方差", "标准差"],
            answer_patterns: &[],
            cause_label: "方差公式应用错误",
            cause_detail: "方差是各数据与平均数差的平方的平均数，标准差是方差的算术平方根。",
            kg_code: "MATH.SX.STAT.STAT.STAT.T02",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["概率", "随机"],
            answer_patterns: &[r"\d+\s*/\s*\d+"],
            cause_label: "古典概型基本事件计数遗漏",
            cause_detail: "古典概型 P(A)=A 包含的基本事件数/基本事件总数，列举时需不重不漏。",
            kg_code: "MATH.SX.STAT.STAT.PROB.T03",
            confidence: 0.55,
        },
        AttributionRule {
            stem_keywords: &["条件概率"],
            answer_patterns: &[],
            cause_label: "条件概率公式理解偏差",
            cause_detail: "P(B|A)=P(AB)/P(A)，注意样本空间已缩减为事件 A。",
            kg_code: "MATH.SX.STAT.STAT.PROB.T05",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["等差数列"],
            answer_patterns: &[],
            cause_label: "等差数列通项与求和公式混淆",
            cause_detail: "an=a1+(n-1)d，Sn=n(a1+an)/2=na1+n(n-1)d/2，注意首项与项数对应。",
            kg_code: "MATH.SX.CALC.CALC.SEQ.T02",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["等比数列"],
            answer_patterns: &[],
            cause_label: "等比数列公比讨论不完整",
            cause_detail: "等比数列求和需讨论 q=1 与 q≠1；等比中项有正负两解需注意。",
            kg_code: "MATH.SX.CALC.CALC.SEQ.T03",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["错位相减"],
            answer_patterns: &[],
            cause_label: "错位相减法运算出错",
            cause_detail: "错位相减注意对齐幂次，相减后中间项构成等比数列，首末项符号易错。",
            kg_code: "MATH.SX.CALC.CALC.SEQ.T04",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["导数", "切线"],
            answer_patterns: &[],
            cause_label: "导数几何意义理解偏差",
            cause_detail: "导数 f'(x0) 是切线斜率，切点既在曲线上也在切线上，\"在点\"与\"过点\"切线有别。",
            kg_code: "MATH.SX.CALC.CALC.DERIV.T04",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["极值", "单调区间"],
            answer_patterns: &[],
            cause_label: "导数求极值忽略定义域与检验",
            cause_detail: "f'(x)=0 的点不一定是极值点，需检验两侧导数符号；注意函数定义域端点。",
            kg_code: "MATH.SX.CALC.CALC.DERIV.T06",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["数学归纳法"],
            answer_patterns: &[],
            cause_label: "数学归纳法递推步骤不完整",
            cause_detail: "归纳法需验证 n=n0 成立，并由 n=k 假设推 n=k+1，两步缺一不可。",
            kg_code: "MATH.SX.CALC.CALC.PROOF.T02",
            confidence: 0.6,
        },
        AttributionRule {
            stem_keywords: &["复数", "虚部", "实部"],
            answer_patterns: &[r"\bi\b"],
            cause_label: "复数运算符号错误",
            cause_detail: "i²=-1，复数除法需乘以共轭复数；实部虚部均为实数（虚部不含 i）。",
            kg_code: "MATH.SX.CALC.CALC.PROOF.T05",
            confidence: 0.55,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_count_meets_bootstrap_target() {
        // Q-8：首批 40 条骨架（目标 50-100 条，可扩充）
        let engine = RuleEngine::new();
        assert!(
            engine.rule_count() >= 40,
            "首批规则应 ≥40 条，实际 {}",
            engine.rule_count()
        );
    }

    #[test]
    fn scenario_a_quadratic_monotonicity_hit() {
        // PRD 场景 A：二次函数区间单调性错题，学生答 a≥1
        let engine = RuleEngine::new();
        let hit = engine
            .evaluate(
                "已知 f(x)=x²-2ax+3 在 [1,2] 上单调递减，求 a 的取值范围",
                "a≥1",
            )
            .expect("场景 A 题干应命中规则");
        assert_eq!(hit.cause_label, "对称轴与区间位置关系理解偏差");
        assert_eq!(hit.kg_code, "MATH.SX.FUNC.FUNC.QUAD.T03");
        assert!(hit.confidence > 0.7 && hit.confidence <= 0.9);
    }

    #[test]
    fn answer_hit_weighs_double() {
        // 题干仅命中 1 关键词（判别式）：作答命中正则 → 得分 1+2=3
        let engine = RuleEngine::new();
        let with_answer = engine
            .evaluate("请用判别式判断根数", "△=b^2-4ac")
            .expect("应命中判别式规则");
        assert_eq!(with_answer.cause_label, "判别式使用条件记忆不清");
        // 置信度被命中强度微调：0.62 + 0.02×3
        assert!((with_answer.confidence - 0.68).abs() < 1e-9);
        // 同题干无作答命中：得分 1 → 0.62 + 0.02
        let stem_only = engine.evaluate("请用判别式判断根数", "不会").unwrap();
        assert!((stem_only.confidence - 0.64).abs() < 1e-9);
    }

    #[test]
    fn no_keyword_no_hit() {
        let engine = RuleEngine::new();
        assert!(engine.evaluate("今天天气晴朗适合郊游", "不知道").is_none());
    }

    #[test]
    fn confidence_capped_at_0_9() {
        // 构造多命中：题干命中 4 关键词 + 作答命中 2 正则 → 0.78 + 0.16 应被封顶 0.9
        let engine = RuleEngine::new();
        let hit = engine
            .evaluate("单调递减与单调递增的单调性由对称轴决定", "a≥1 还是 a≤0")
            .expect("应命中");
        assert!(hit.confidence <= 0.9, "规则初判置信度封顶 0.9");
    }

    #[test]
    fn keyword_match_is_case_insensitive() {
        let engine = RuleEngine::new();
        let hit = engine.evaluate("研究函数的奇偶性", "f(-X)=F(x)");
        assert!(hit.is_some(), "大小写不敏感匹配");
        assert_eq!(hit.unwrap().cause_label, "函数奇偶性判定忽略定义域");
    }
}
