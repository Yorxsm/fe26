"use client";

import { useEffect } from "react";
import { markQuest, type QuestId } from "@/lib/onboarding";

export function QuestPing({ quest }: { quest: QuestId }) {
  useEffect(() => {
    markQuest(quest);
  }, [quest]);
  return null;
}
