"use client";

/**
 * Improvement 9 — Error boundary component.
 *
 * React render errors are silent in production — the component tree
 * disappears and the user sees a blank section with no explanation. This
 * boundary catches render errors, logs them, and shows a recovery UI that
 * tells the member what happened and gives them a way out.
 *
 * Usage (wrap any page section that reads chain or index data):
 *
 *   <ErrorBoundary label="proposal list">
 *     <ProposalTable ... />
 *   </ErrorBoundary>
 *
 * The `label` prop appears in the error message and in any future error
 * reporting, so give it a name a non-engineer can understand.
 */

import React from "react";

interface Props {
  children: React.ReactNode;
  /** Human-readable name for the section being protected. */
  label?: string;
  /** Custom fallback. Receives the error and a reset callback. */
  fallback?: (error: Error, reset: () => void) => React.ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    // In production, wire this to your error reporting service.
    console.error(`[ErrorBoundary] ${this.props.label ?? "unknown"}:`, error, info);
  }

  reset = (): void => {
    this.setState({ error: null });
  };

  render(): React.ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    if (this.props.fallback) {
      return this.props.fallback(error, this.reset);
    }

    return (
      <div className="notice notice--caution" role="alert" style={{ margin: "var(--s-5) 0" }}>
        <strong>Something went wrong{this.props.label ? ` in ${this.props.label}` : ""}.</strong>
        <p style={{ margin: "var(--s-2) 0 0" }}>
          {error.message ?? "An unexpected error occurred."}
        </p>
        <button
          onClick={this.reset}
          style={{
            marginTop: "var(--s-3)",
            padding: "var(--s-2) var(--s-4)",
            cursor: "pointer",
          }}
        >
          Try again
        </button>
      </div>
    );
  }
}

/**
 * Functional wrapper for convenience in server components and layouts.
 *
 * React class components are still the only way to implement error boundaries,
 * but this wrapper lets callers use the familiar JSX shorthand:
 *
 *   <Boundary label="treasury">
 *     <TreasuryPanel />
 *   </Boundary>
 */
export function Boundary({
  children,
  label,
  fallback,
}: Props): React.ReactElement {
  return (
    <ErrorBoundary label={label} fallback={fallback}>
      {children}
    </ErrorBoundary>
  );
}
