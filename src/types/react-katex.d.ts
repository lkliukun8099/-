// react-katex 类型声明（官方未内置 .d.ts）
declare module "react-katex" {
  import * as React from "react";

  export interface KatexProps {
    math?: string;
    children?: string;
    block?: boolean;
    errorColor?: string;
    renderError?: (error: Error) => React.ReactNode;
    as?: keyof JSX.IntrinsicElements;
    className?: string;
    settings?: Record<string, unknown>;
  }

  export const InlineMath: React.FC<KatexProps>;
  export const BlockMath: React.FC<KatexProps>;
}
