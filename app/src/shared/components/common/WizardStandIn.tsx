import React, { memo, useEffect } from "react";
import { RotateCw } from "lucide-react";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { revealWizard } from "@/services/setupService";

/**
 * Placeholder shown in the `main` webview while setup is incomplete.
 *
 * On a first run the real setup flow runs in a dedicated `wizard` webview
 * (constructed by `ensure_wizard_window` at bootstrap). Previously `main` ALSO
 * routed itself into `<WizardRoot />`, so two full copies of the setup flow
 * mounted in two webviews on the same machine — and `main` additionally booted
 * the entire app shell (pipeline listeners, spatial navigation, profiler,
 * notification listeners) behind it.
 *
 * So `main` presents this stand-in and asks the backend to surface the real
 * window instead of duplicating it.
 */
export const WizardStandIn: React.FC = memo(() => {
  useEffect(() => {
    // The wizard window is created during backend bootstrap; if the user is
    // already looking at `main`, bring the setup window forward immediately.
    void revealWizard().catch(() => {});
  }, []);

  return (
    <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))] px-6">
      <div className="flex flex-col items-center gap-4 text-center max-w-md">
        <h1 className="text-[15px] font-display font-black uppercase tracking-[0.16em] text-[rgb(var(--foreground))]">
          {LAYOUT_COPY.setupWindow.title}
        </h1>
        <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
          {LAYOUT_COPY.setupWindow.body}
        </p>
        <button
          type="button"
          onClick={() => void revealWizard().catch(() => {})}
          className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-mono bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] transition-colors duration-150 cursor-pointer"
        >
          <RotateCw size={13} />
          {LAYOUT_COPY.setupWindow.bringForward}
        </button>
      </div>
    </div>
  );
});

WizardStandIn.displayName = "WizardStandIn";