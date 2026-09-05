import { describe, it, expect } from "vitest";
import {
  toCategory,
  isReceipt,
  countsTowardBadge,
  metadataTurnCount,
  type NotificationRecord,
} from "../notificationService";
import { selectBadgeCount } from "@/store/notificationStore";

function record(partial: Partial<NotificationRecord> & { id: string }): NotificationRecord {
  return {
    category: "session_compaction",
    title: "Demo",
    message: "",
    status: "pending",
    session_id: null,
    metadata: "{}",
    is_read: false,
    created_at: Date.now(),
    ...partial,
  };
}

describe("notificationCategories", () => {
  describe("toCategory", () => {
    it("passes known categories through", () => {
      expect(toCategory("model_ready")).toBe("model_ready");
      expect(toCategory("memory_issue")).toBe("memory_issue");
      expect(toCategory("storage_health")).toBe("storage_health");
    });

    it("maps unknown future categories to the default presentation", () => {
      expect(toCategory("something_new")).toBe("session_compaction");
      expect(toCategory("")).toBe("session_compaction");
    });
  });

  describe("isReceipt / countsTowardBadge", () => {
    it("treats completed and failed as receipts", () => {
      expect(isReceipt(record({ id: "a", status: "completed" }))).toBe(true);
      expect(isReceipt(record({ id: "b", status: "failed" }))).toBe(true);
      expect(isReceipt(record({ id: "c", status: "pending" }))).toBe(false);
      expect(isReceipt(record({ id: "d", status: "in_progress" }))).toBe(false);
    });

    it("badges unread actionable items only", () => {
      expect(countsTowardBadge(record({ id: "a" }))).toBe(true);
      expect(countsTowardBadge(record({ id: "b", is_read: true }))).toBe(false);
      expect(countsTowardBadge(record({ id: "c", status: "completed" }))).toBe(false);
      expect(countsTowardBadge(record({ id: "d", status: "failed" }))).toBe(false);
      expect(countsTowardBadge(record({ id: "e", status: "dismissed" }))).toBe(false);
    });

    it("selectBadgeCount aggregates the store", () => {
      const state = {
        notifications: [
          record({ id: "a" }),
          record({ id: "b", is_read: true }),
          record({ id: "c", status: "completed" }),
          record({ id: "d", status: "failed" }),
        ],
      };
      expect(selectBadgeCount(state as never)).toBe(1);
    });
  });

  describe("metadataTurnCount", () => {
    it("reads uncompacted_turns from metadata JSON", () => {
      expect(
        metadataTurnCount(record({ id: "a", metadata: '{"uncompacted_turns": 4}' }))
      ).toBe(4);
    });

    it("returns null for missing or malformed metadata", () => {
      expect(metadataTurnCount(record({ id: "a", metadata: "{}" }))).toBeNull();
      expect(metadataTurnCount(record({ id: "b", metadata: "not json" }))).toBeNull();
      expect(
        metadataTurnCount(record({ id: "c", metadata: '{"uncompacted_turns": "4"}' }))
      ).toBeNull();
    });
  });
});
