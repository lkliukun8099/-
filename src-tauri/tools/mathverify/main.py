#!/usr/bin/env python3
"""mathverify：SymPy 符号校验 sidecar（stdio JSON-RPC，逐行 JSON）。

打包：PyInstaller --onefile main.py（产出 mathverify[.exe]，约 40MB，可选组件 U-5）

协议：
  请求  {"id": 1, "method": "<method>", "params": {...}}   （单行 JSON）
  响应  {"id": 1, "result": {...}}  或  {"id": 1, "error": "..."}

方法：
  check_expression          {expr}                       -> {ok, detail, simplified?}
  verify_equation_solution  {equation, solution}         -> {ok, detail}
  evaluate                  {expr, vars?}                -> {ok, detail, value?}
"""
from __future__ import annotations

import json
import re
import sys

try:
    import sympy
    from sympy.parsing.sympy_parser import (
        parse_expr,
        standard_transformations,
        implicit_multiplication_application,
        convert_xor,
    )
except ImportError:  # 打包环境缺失依赖时给出明确错误
    print(json.dumps({"id": 0, "error": "sympy 未安装，mathverify 不可用"}), flush=True)
    sys.exit(1)

TRANSFORMS = standard_transformations + (
    implicit_multiplication_application,
    convert_xor,
)


def _preprocess(expr: str) -> str:
    """宽松预处理：兼容常见 LaTeX/中文数学记号。"""
    s = expr.strip()
    # 常见 LaTeX 记号降级
    s = s.replace("\\left", "").replace("\\right", "")
    s = s.replace("\\cdot", "*").replace("\\times", "*").replace("\\div", "/")
    s = re.sub(r"\\frac\{([^{}]+)\}\{([^{}]+)\}", r"(\1)/(\2)", s)
    s = re.sub(r"\\sqrt\{([^{}]+)\}", r"sqrt(\1)", s)
    s = s.replace("\\pi", "pi").replace("π", "pi")
    s = s.replace("²", "**2").replace("³", "**3")
    s = s.replace("^", "**")
    s = s.replace("，", ",").replace("（", "(").replace("）", ")")
    # 去掉 LaTeX 环境残留的反斜杠命令（保守处理）
    s = re.sub(r"\\(geq?|geqslant)", ">=", s)
    s = re.sub(r"\\(leq?|leqslant)", "<=", s)
    s = re.sub(r"\\neq?", "!=", s)
    return s


def _parse(expr: str):
    return parse_expr(_preprocess(expr), transformations=TRANSFORMS, evaluate=True)


def check_expression(params: dict) -> dict:
    """表达式可解析性 + 化简可行性检查。"""
    expr = params.get("expr", "")
    if not expr:
        return {"ok": False, "detail": "表达式为空"}
    try:
        parsed = _parse(expr)
        simplified = sympy.simplify(parsed)
        if simplified in (sympy.nan, sympy.zoo, sympy.oo, -sympy.oo) and "oo" not in expr:
            return {"ok": False, "detail": f"表达式化简结果异常: {simplified}"}
        return {"ok": True, "detail": "表达式可解析", "simplified": str(simplified)}
    except Exception as e:  # noqa: BLE001 - sidecar 需兜底一切解析异常
        return {"ok": False, "detail": f"表达式无法解析: {e}"}


def verify_equation_solution(params: dict) -> dict:
    """代入验证：solution 是否满足 equation。

    equation 形如 "x**2-5*x+6=0"，solution 形如 "x=2"（或多解 "x=2,x=3"）。
    """
    equation = params.get("equation", "")
    solution = params.get("solution", "")
    if "=" not in equation:
        return {"ok": False, "detail": "题干未提取到方程"}
    try:
        lhs_s, rhs_s = equation.split("=", 1)
        lhs, rhs = _parse(lhs_s), _parse(rhs_s)
        eq = sympy.Eq(lhs, rhs)
        free = list(eq.free_symbols)
        if not free:
            # 无变量方程：直接判断真假
            truth = bool(sympy.simplify(lhs - rhs) == 0)
            return {"ok": truth, "detail": "常量方程恒成立" if truth else "常量方程不成立"}
        var = free[0]

        # 解析解：支持 "x=2"、"2"、"x=2,x=3" 形式
        candidates: list = []
        for part in re.split(r"[,，]", solution):
            part = part.strip()
            if not part:
                continue
            if "=" in part:
                _, v = part.split("=", 1)
                candidates.append(_parse(v))
            else:
                candidates.append(_parse(part))
        if not candidates:
            return {"ok": False, "detail": "未解析到候选解"}

        wrong = []
        for cand in candidates:
            residual = sympy.simplify((lhs - rhs).subs(var, cand))
            if residual != 0:
                wrong.append(str(cand))
        if wrong:
            return {"ok": False, "detail": f"代入验证失败: {', '.join(wrong)} 不满足方程"}
        return {"ok": True, "detail": f"{len(candidates)} 个解代入验证通过"}
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "detail": f"验证过程出错: {e}"}


def evaluate(params: dict) -> dict:
    """求值：expr 在 vars 代入下的数值。"""
    expr = params.get("expr", "")
    var_map = params.get("vars", {}) or {}
    try:
        parsed = _parse(expr)
        subs = {sympy.Symbol(k): _parse(str(v)) for k, v in var_map.items()}
        value = sympy.N(parsed.subs(subs), 10)
        return {"ok": True, "detail": "求值成功", "value": str(value)}
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "detail": f"求值失败: {e}"}


METHODS = {
    "check_expression": check_expression,
    "verify_equation_solution": verify_equation_solution,
    "evaluate": evaluate,
}


def main() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            req_id = req.get("id", 0)
            method = req.get("method", "")
            params = req.get("params", {}) or {}
            handler = METHODS.get(method)
            if handler is None:
                resp = {"id": req_id, "error": f"未知方法: {method}"}
            else:
                resp = {"id": req_id, "result": handler(params)}
        except Exception as e:  # noqa: BLE001 - 任何异常都不应使 sidecar 崩溃
            resp = {"id": 0, "error": f"请求处理异常: {e}"}
        print(json.dumps(resp, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
