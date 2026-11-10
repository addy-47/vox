import { describe, it, expect, afterEach, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DetailPanel } from "@/shared/components/home/recall/DetailPanel";
import type { SessionRow, TurnRow } from "@/services/persistenceService";

afterEach(cleanup);

const MOCK_SESSION: SessionRow = {
  id: 101,
  project_id: "vox-core",
  title: "Architecture Discussion",
  created_at: 1700000000000,
  updated_at: 1700000100000,
  is_pinned: false,
  deleted_at: null,
  turn_count: 2,
  first_message: "Hello Vox",
  uncompacted_turns: 2,
};

const MOCK_TURNS: TurnRow[] = [
  {
    id: 1,
    session_id: 101,
    turn_id: 1,
    user_text: "Hello Vox",
    assistant_text: "Hello! How can I help you?",
    created_at: 1700000010000,
  },
];

describe("DetailPanel render & state stability", () => {
  it("renders safely when closed with session=null without infinite render loop", () => {
    // This directly tests the regression where session?.id !== prevSessionId caused
    // undefined !== null -> infinite renderWithHooksAgain on initial Home render.
    expect(() => {
      render(
        <DetailPanel
          open={false}
          session={null}
          turns={[]}
          loading={false}
          onClose={vi.fn()}
        />
      );
    }).not.toThrow();
  });

  it("renders session information when opened with a valid session", () => {
    render(
      <DetailPanel
        open={true}
        session={MOCK_SESSION}
        turns={MOCK_TURNS}
        loading={false}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText("Architecture Discussion")).toBeDefined();
    expect(screen.getByText("Hello Vox")).toBeDefined();
    expect(screen.getByText("Hello! How can I help you?")).toBeDefined();
  });

  it("handles session prop transitions without cascading re-renders", () => {
    const { rerender } = render(
      <DetailPanel
        open={false}
        session={null}
        turns={[]}
        loading={false}
        onClose={vi.fn()}
      />
    );

    // Transition from null to active session
    rerender(
      <DetailPanel
        open={true}
        session={MOCK_SESSION}
        turns={MOCK_TURNS}
        loading={false}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText("Architecture Discussion")).toBeDefined();

    // Transition to a different session
    const NEXT_SESSION: SessionRow = {
      ...MOCK_SESSION,
      id: 102,
      title: "Project Review",
    };

    rerender(
      <DetailPanel
        open={true}
        session={NEXT_SESSION}
        turns={MOCK_TURNS}
        loading={false}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText("Project Review")).toBeDefined();
  });
});
