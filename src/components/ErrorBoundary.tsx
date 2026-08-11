import { Component, type ErrorInfo, type ReactNode } from "react";

import { captureRecoverableError } from "../lib/diagnostics";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  message: string | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, message: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    const message =
      error instanceof Error ? error.message : "Une erreur inattendue est survenue.";
    return { hasError: true, message };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    void captureRecoverableError(
      `${error.message} @ ${info.componentStack?.trim().slice(0, 200) ?? "unknown"}`,
      "react",
      "react_error_boundary",
    );
  }

  private handleReload = () => {
    window.location.reload();
  };

  private handleReset = () => {
    this.setState({ hasError: false, message: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary" role="alert">
          <p className="lm-kicker">Erreur</p>
          <h1>La Minute a rencontré un problème.</h1>
          <p className="lm-subtle">
            L&apos;interface a été isolée. Vous pouvez réessayer ou recharger l&apos;application.
            Aucune donnée de réunion n&apos;a été envoyée.
          </p>
          {this.state.message ? <p className="error-boundary__detail mono">{this.state.message}</p> : null}
          <div className="row controls">
            <button type="button" className="lm-btn lm-btn--primary" onClick={this.handleReset}>
              Réessayer
            </button>
            <button type="button" className="lm-btn" onClick={this.handleReload}>
              Recharger
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
