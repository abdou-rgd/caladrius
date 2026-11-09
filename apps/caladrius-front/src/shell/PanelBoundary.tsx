import { Component, type ErrorInfo, type ReactNode } from 'react';

/** An error boundary per panel: a panel that fails says so and offers to try again; the others keep working. */
export class PanelBoundary extends Component<{ name: string; children: ReactNode }, { error: string | null }> {
  override state: { error: string | null } = { error: null };

  static getDerivedStateFromError(error: unknown): { error: string } {
    return { error: error instanceof Error ? error.message : String(error) };
  }

  override componentDidCatch(_error: Error, _info: ErrorInfo): void {}

  override render(): ReactNode {
    if (this.state.error === null) return this.props.children;
    return (
      <div className="panel" role="alert">
        <div className="banner error">
          The {this.props.name} panel could not be drawn: {this.state.error}
        </div>
        <div>
          <button className="btn" onClick={() => this.setState({ error: null })}>
            Try again
          </button>
        </div>
      </div>
    );
  }
}
