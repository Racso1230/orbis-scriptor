import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface Props {
  children: ReactNode;
  /** Changing this value clears the error (e.g. the current route path). */
  resetKey?: string;
  /** Use a compact fallback that fits inside a page area instead of the whole window. */
  inline?: boolean;
}

interface State {
  error: Error | null;
}

/**
 * Catches render errors in its children and shows a friendly fallback.
 *
 * React only supports this as a class component (there is no hook for it).
 * `getDerivedStateFromError` stores the error so the next render shows the
 * fallback; `componentDidCatch` is where we log it.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unhandled UI error", error, info.componentStack);
  }

  componentDidUpdate(prev: Props) {
    if (this.state.error && prev.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  private backToWelcome = () => {
    // Hash history: "#/" is the welcome route (it redirects into the world if one is open).
    window.location.hash = "#/";
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <div
        role="alert"
        className={
          this.props.inline
            ? "flex flex-1 items-center justify-center p-8"
            : "flex h-screen items-center justify-center p-8"
        }
      >
        <div className="flex w-full max-w-lg flex-col gap-4">
          <div>
            <h1 className="text-xl font-semibold">Something went wrong</h1>
            <p className="text-sm text-muted-foreground">
              This part of the app hit an unexpected error. Your saved data is not affected.
            </p>
          </div>
          <details className="rounded-md border p-3 text-xs">
            <summary className="cursor-pointer text-sm">Error details</summary>
            <pre className="mt-2 overflow-auto whitespace-pre-wrap">
              {error.name}: {error.message}
              {error.stack ? `\n\n${error.stack}` : ""}
            </pre>
          </details>
          <div className="flex gap-2">
            <Button onClick={() => window.location.reload()}>Reload</Button>
            <Button variant="outline" onClick={this.backToWelcome}>
              Back to welcome
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
