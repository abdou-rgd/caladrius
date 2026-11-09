import { Component, type ReactNode } from 'react';

interface Props {
  label: string;
  children: ReactNode;
}

interface State {
  failed: boolean;
}

/** A table that cannot be drawn says so in one sentence; the rest of the page keeps working. */
export class TableBoundary extends Component<Props, State> {
  override state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  override render(): ReactNode {
    if (this.state.failed) {
      return (
        <div className="ct-error" role="alert">
          The table “{this.props.label}” could not be drawn. Check the values it was given, then reload the results.
        </div>
      );
    }
    return this.props.children;
  }
}
