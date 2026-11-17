#!/usr/bin/env node
/**
 * 苏教版 K12 数学三级知识图谱种子数据生成器。
 * 用法：node scripts/gen_kg_seed.cjs
 * 产物：src-tauri/resources/kg_seed/math_sujiao.json
 *
 * 说明：三级节点名称为教研占位数据（架构文档 U-1），后续可由教研团队
 * 按同一 JSON 结构扩充/修订，无需改代码。
 */
const fs = require("fs");
const path = require("path");

// 一级领域 -> 二级章节 -> 三级知识点
const TAXONOMY = {
  "MATH.SX.NUM|数与代数": {
    "NUM.RAT|有理数": [
      "数轴与有理数大小比较",
      "相反数与绝对值",
      "有理数加减运算",
      "有理数乘除与乘方",
      "科学记数法与近似数",
      "有理数混合运算",
      "数轴动点问题",
    ],
    "NUM.REAL|实数": [
      "平方根与立方根",
      "实数概念与分类",
      "二次根式化简",
      "二次根式四则运算",
      "实数大小估算",
      "二次根式化简求值",
    ],
    "NUM.POLY|整式与分式": [
      "整式的加减",
      "幂的运算法则",
      "整式的乘除",
      "乘法公式（平方差/完全平方）",
      "因式分解（提公因式）",
      "因式分解（公式法与十字相乘）",
      "分式基本性质与约分",
      "分式的四则运算",
      "整式化简求值",
    ],
    "NUM.EQ|方程与不等式": [
      "一元一次方程解法",
      "一元一次方程应用",
      "二元一次方程组解法",
      "二元一次方程组应用",
      "一元二次方程直接开平方法",
      "一元二次方程配方法",
      "一元二次方程求根公式",
      "判别式与根的情况",
      "根与系数的关系（韦达定理）",
      "一元一次不等式（组）",
      "不等式（组）整数解问题",
      "分式方程及增根",
      "一元二次方程应用（增长率/面积）",
      "含参方程解的讨论",
    ],
  },
  "MATH.SX.FUNC|函数": {
    "FUNC.LINEAR|一次函数": [
      "平面直角坐标系",
      "函数概念与表示",
      "一次函数图象与性质",
      "待定系数法求解析式",
      "一次函数与方程不等式",
      "一次函数实际应用",
      "一次函数图象共存问题",
    ],
    "FUNC.INV|反比例函数": [
      "反比例函数图象与性质",
      "比例系数 k 的几何意义",
      "反比例函数与一次函数综合",
      "反比例函数实际应用",
      "反比例函数与几何面积",
    ],
    "FUNC.QUAD|二次函数": [
      "二次函数图象与性质",
      "顶点式与对称轴",
      "二次函数单调性（对称轴与区间关系）",
      "二次函数最值问题",
      "二次函数与一元二次方程",
      "抛物线平移变换",
      "二次函数实际应用（利润/面积）",
      "含参二次函数讨论",
      "二次函数图象与系数关系",
    ],
    "FUNC.HIGH|基本初等函数": [
      "指数运算与指数函数",
      "对数运算与对数函数",
      "幂函数图象与性质",
      "函数单调性判定",
      "函数奇偶性判定",
      "函数零点与二分法",
      "函数图象变换",
      "分段函数求值与图象",
      "复合函数单调性",
    ],
    "FUNC.MODEL|函数建模": [
      "一次函数建模",
      "二次函数建模",
      "分段函数建模",
      "指数对数模型应用",
    ],
  },
  "MATH.SX.TRIG|三角与向量": {
    "TRIG.ACUTE|锐角三角函数": [
      "正弦余弦正切定义",
      "特殊角三角函数值",
      "解直角三角形",
      "仰角俯角测量问题",
      "坡度坡角问题",
      "方向角问题",
    ],
    "TRIG.FUNC|三角函数": [
      "任意角与弧度制",
      "同角三角函数关系",
      "诱导公式",
      "三角函数图象与性质",
      "三角恒等变换",
      "正弦定理",
      "余弦定理",
      "解三角形综合应用",
      "三角函数最值问题",
    ],
    "TRIG.VEC|平面向量": [
      "向量线性运算",
      "向量共线定理",
      "向量数量积",
      "向量坐标运算",
      "向量平行与垂直判定",
      "向量模长与夹角",
      "向量在几何中的应用",
    ],
  },
  "MATH.SX.GEO|图形与几何": {
    "GEO.TRI|三角形": [
      "三角形三边关系",
      "全等三角形判定",
      "全等三角形性质应用",
      "等腰三角形性质与判定",
      "直角三角形与勾股定理",
      "勾股定理逆定理",
      "三角形中位线",
      "相似三角形判定",
      "相似三角形性质与计算",
      "手拉手模型与旋转全等",
    ],
    "GEO.QUAD|四边形": [
      "平行四边形性质与判定",
      "矩形性质与判定",
      "菱形性质与判定",
      "正方形性质与判定",
      "梯形与中位线",
      "中点四边形",
      "特殊四边形综合证明",
    ],
    "GEO.TRANS|图形变换": [
      "轴对称与最短路径",
      "平移变换",
      "旋转变换",
      "中心对称",
      "位似变换",
    ],
    "GEO.CIRCLE|圆": [
      "垂径定理",
      "弧弦圆心角关系",
      "圆周角定理及推论",
      "点直线与圆的位置关系",
      "切线的判定与性质",
      "切线长定理",
      "弧长与扇形面积",
      "圆与三角形四边形综合",
      "圆中常见辅助线作法",
    ],
    "GEO.SOLID|立体几何": [
      "几何体三视图",
      "柱锥球的表面积",
      "柱锥球的体积",
      "空间点线面位置关系",
      "直线与平面平行判定",
      "直线与平面垂直判定",
      "平面与平面平行垂直",
      "空间几何体截面问题",
    ],
    "GEO.ANALYTIC|解析几何": [
      "直线斜率与倾斜角",
      "直线方程五种形式",
      "两直线位置关系",
      "点到直线距离",
      "圆的方程",
      "直线与圆的位置关系",
      "椭圆标准方程与性质",
      "双曲线标准方程与性质",
      "抛物线标准方程与性质",
      "圆锥曲线弦长问题",
      "离心率问题",
      "定点定值问题",
    ],
  },
  "MATH.SX.STAT|统计与概率": {
    "STAT.STAT|统计": [
      "平均数中位数众数",
      "方差与标准差",
      "频数分布直方图",
      "扇形图与折线图",
      "简单随机抽样",
      "分层抽样",
      "用样本估计总体",
      "数据波动分析与决策",
    ],
    "STAT.PROB|概率": [
      "随机事件与概率意义",
      "列举法求概率",
      "古典概型",
      "几何概型",
      "条件概率",
      "相互独立事件概率",
      "离散型随机变量分布列",
      "数学期望与方差",
      "二项分布初步",
    ],
  },
  "MATH.SX.CALC|导数数列与推理": {
    "CALC.SEQ|数列": [
      "数列概念与通项",
      "等差数列通项与前n项和",
      "等比数列通项与前n项和",
      "数列求和（错位相减）",
      "数列求和（裂项相消）",
      "递推数列求通项",
      "数列与不等式综合",
    ],
    "CALC.DERIV|导数及其应用": [
      "导数概念与几何意义",
      "基本初等函数求导",
      "导数四则运算法则",
      "利用导数求切线",
      "利用导数研究单调性",
      "极值与最值",
      "恒成立与存在性问题",
      "导数与不等式证明",
      "函数零点个数问题",
    ],
    "CALC.PROOF|推理证明与复数": [
      "合情推理与演绎推理",
      "数学归纳法",
      "基本不等式求最值",
      "不等式证明（比较/综合/分析）",
      "复数概念与四则运算",
      "复数的几何意义",
      "复数模与轨迹问题",
    ],
    "CALC.INEQ|不等式选讲": [
      "绝对值不等式解法",
      "柯西不等式初步",
      "排序不等式初步",
    ],
  },
};

const nodes = [];
const edges = [];
let l3Count = 0;

for (const [l1Key, l2Map] of Object.entries(TAXONOMY)) {
  const [l1Code, l1Name] = l1Key.split("|");
  const l1 = { code: l1Code, name: l1Name, level: 1, children: [] };
  const l2Entries = Object.entries(l2Map);

  l2Entries.forEach(([l2Key, topics], l2Idx) => {
    const [l2Suffix, l2Name] = l2Key.split("|");
    const l2Code = `${l1Code}.${l2Suffix}`;
    const l2 = { code: l2Code, name: l2Name, level: 2, children: [] };

    topics.forEach((topic, tIdx) => {
      const tCode = `${l2Code}.T${String(tIdx + 1).padStart(2, "0")}`;
      l2.children.push({ code: tCode, name: topic, level: 3 });
      l3Count += 1;
      // 同一二级章节内：前一知识点是后一知识点的先修
      if (tIdx > 0) {
        const prev = `${l2Code}.T${String(tIdx).padStart(2, "0")}`;
        edges.push({ from: prev, to: tCode, relation: "prerequisite" });
      }
    });

    // 同一一级领域内：前一章节的末节点与下一章节首节点建立 related 关联
    if (l2Idx > 0) {
      edges.push({
        from: `${l1Code}.${l2Entries[l2Idx - 1][0].split("|")[0]}.T01`,
        to: `${l2Code}.T01`,
        relation: "related",
      });
    }
    l1.children.push(l2);
  });
  nodes.push(l1);
}

// 跨领域关键先修关系（体现图谱边能力）
edges.push(
  { from: "MATH.SX.NUM.NUM.EQ.T07", to: "MATH.SX.FUNC.FUNC.QUAD.T05", relation: "prerequisite" },
  { from: "MATH.SX.FUNC.FUNC.QUAD.T03", to: "MATH.SX.CALC.CALC.DERIV.T05", relation: "prerequisite" },
  { from: "MATH.SX.NUM.NUM.POLY.T06", to: "MATH.SX.NUM.NUM.EQ.T06", relation: "prerequisite" },
  { from: "MATH.SX.GEO.GEO.TRI.T05", to: "MATH.SX.TRIG.TRIG.ACUTE.T03", relation: "prerequisite" },
  { from: "MATH.SX.FUNC.FUNC.LINEAR.T04", to: "MATH.SX.GEO.GEO.ANALYTIC.T02", relation: "prerequisite" },
  { from: "MATH.SX.TRIG.TRIG.FUNC.T06", to: "MATH.SX.TRIG.TRIG.FUNC.T08", relation: "prerequisite" },
  { from: "MATH.SX.NUM.NUM.REAL.T03", to: "MATH.SX.NUM.NUM.POLY.T04", relation: "prerequisite" }
);

const seed = {
  meta: {
    name: "苏教版 K12 数学主干知识图谱（三级）",
    version: "1.0.0",
    textbook: "sujiao",
    note: "教研占位数据，可按同结构扩充（架构文档 U-1）",
    l3NodeCount: l3Count,
  },
  subject: { code: "math", name: "数学" },
  nodes,
  edges,
};

const out = path.join(__dirname, "..", "src-tauri", "resources", "kg_seed", "math_sujiao.json");
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(seed, null, 2), "utf8");
console.log(`已生成 ${out}`);
console.log(`一级 ${nodes.length} 个，三级节点 ${l3Count} 个，边 ${edges.length} 条`);
