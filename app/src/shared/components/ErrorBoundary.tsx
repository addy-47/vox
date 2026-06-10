import React from 'react';

interface ErrorBoundaryProps {
  children: React.ReactNode;
  fallback?: React.ReactNode;
  name?: string;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error(`[ErrorBoundary:${this.props.name || 'unknown'}]`, error);
    console.error('Component stack:', info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div style={{
          padding: 16,
          margin: 8,
          borderRadius: 8,
          background: 'rgba(220,38,38,0.1)',
          border: '1px solid rgba(220,38,38,0.3)',
          color: 'rgb(var(--foreground))',
          fontSize: 12,
          fontFamily: 'monospace',
        }}>
          <div style={{ fontWeight: 'bold', marginBottom: 8, color: '#dc2626' }}>
            ⚠️ Render Error: {this.props.name || 'Component'}
          </div>
          <div style={{ marginBottom: 4, opacity: 0.8 }}>
            {this.state.error?.message || 'Unknown error'}
          </div>
          {this.state.error?.stack && (
            <details>
              <summary style={{ cursor: 'pointer', opacity: 0.6, marginTop: 4 }}>Stack trace</summary>
              <pre style={{ fontSize: 10, marginTop: 4, whiteSpace: 'pre-wrap', opacity: 0.5 }}>
                {this.state.error.stack}
              </pre>
            </details>
          )}
        </div>
      );
    }
    return this.props.children;
  }
}
