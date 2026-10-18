import { createContext, useContext } from "react";

/**
 * Whether the enclosing EdgePanel is open.
 *
 * `EdgePanel` keeps `children` mounted while the panel is closed, so a panel
 * body with an expensive subtree must not keep fetching or re-rendering in the
 * background. Read this in the body and gate its data layer on it.
 *
 * The default is `true`, so a body rendered outside an EdgePanel still works.
 *
 * Lives outside `EdgePanel.tsx` on purpose: that file exports a component, and
 * adding a hook export breaks its fast-refresh boundary.
 */
export const EdgePanelActiveContext = createContext(true);

export function useEdgePanelActive(): boolean {
  return useContext(EdgePanelActiveContext);
}
