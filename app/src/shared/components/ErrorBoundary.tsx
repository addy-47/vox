import React from 'react';
import { cn } from '@/shared/lib/utils';

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

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  handleGoHome = () => {
    this.setState({ hasError: false, error: null });
    // Use history API directly since ErrorBoundary may not be inside Router
    window.history.pushState(null, '', '/');
    window.dispatchEvent(new PopStateEvent('popstate'));
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      const name = this.props.name || 'Component';

      return (
        <div className="flex items-center justify-center h-full w-full p-8">
          <div className={cn(
            "glass-card glass-base max-w-md w-full p-8 text-center space-y-6"
          )}>
            {/* Error Icon */}
            <div className="mx-auto w-14 h-14 rounded-2xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 flex items-center justify-center">
              <svg className="w-7 h-7 text-[rgb(var(--accent))]" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>

            {/* Error Title */}
            <div>
              <h2 className="text-lg font-black text-[rgb(var(--foreground))] uppercase tracking-tight mb-2">
                Render Error
              </h2>
              <p className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em]">
                {name}
              </p>
            </div>

            {/* Error Message */}
            <div className="glass-whisper glass-base px-4 py-3 text-left">
              <p className="text-xs font-mono text-[rgb(var(--foreground-muted))] leading-relaxed break-words">
                {this.state.error?.message || 'An unexpected error occurred'}
              </p>
            </div>

            {/* Stack Trace */}
            {this.state.error?.stack && (
              <details className="text-left">
                <summary className="text-[11px] font-bold text-[rgb(var(--foreground-muted))]/50 uppercase tracking-widest cursor-pointer hover:text-[rgb(var(--foreground-muted))] transition-colors">
                  Stack Trace
                </summary>
                <pre className="mt-2 glass-whisper glass-base p-3 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 leading-relaxed overflow-auto max-h-[160px] custom-scrollbar whitespace-pre-wrap">
                  {this.state.error.stack}
                </pre>
              </details>
            )}

            {/* Actions */}
            <div className="flex gap-3 pt-2">
              <button
                onClick={this.handleGoHome}
                className="flex-1 py-3 text-[11px] font-black uppercase tracking-[0.3em] glass-whisper glass-base text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
              >
                Home
              </button>
              <button
                onClick={this.handleRetry}
                className="flex-1 py-3 text-[11px] font-black uppercase tracking-[0.3em] glass-card glass-base hover:border-[rgb(var(--accent))]/70 transition-all active:scale-[0.98]"
              >
                Retry
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
