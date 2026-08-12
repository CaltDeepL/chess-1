import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

// Reactのレンダリング中に投げられた例外を捕まえる最後の砦。
// クラスコンポーネントでしか実装できない(getDerivedStateFromError/componentDidCatch)。
// フォールバックUIは外側のAuthProvider/ToastProviderより上に置くため、
// それらの状態には一切依存しない(プロバイダ自体が壊れても表示できる)。
export default class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unhandled render error:", error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary">
          <div className="error-boundary-card">
            <h1>予期しないエラーが発生しました</h1>
            <p>ページを再読み込みすると解決する場合があります。</p>
            <button onClick={() => window.location.reload()}>再読み込み</button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
