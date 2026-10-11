import React from "react";
import { Sparkles, BookOpen, GraduationCap } from "lucide-react";
import { LearnTab } from "@/types";

export interface LearnTabsProps {
  activeTab: LearnTab;
  onTabChange: (tab: LearnTab) => void;
  conceptCount?: number;
  hasSelectedConcept?: boolean;
  hasLesson?: boolean;
}

export const LearnTabs: React.FC<LearnTabsProps> = ({
  activeTab,
  onTabChange,
  conceptCount = 0,
  hasSelectedConcept = false,
  hasLesson = false,
}) => {
  return (
    <div className="learn-nav-header">
      <div className="learn-tabs">
        <button
          className={`learn-tab-btn ${activeTab === "concepts" ? "active" : ""}`}
          onClick={() => onTabChange("concepts")}
        >
          <Sparkles className="w-4 h-4" />
          Key Concepts
          {conceptCount > 0 && (
            <span className="learn-tab-badge">{conceptCount}</span>
          )}
        </button>

        <button
          className={`learn-tab-btn ${activeTab === "excerpts" ? "active" : ""}`}
          onClick={() => onTabChange("excerpts")}
        >
          <BookOpen className="w-4 h-4" />
          Excerpts
          {hasSelectedConcept && <span className="learn-tab-dot" />}
        </button>

        <button
          className={`learn-tab-btn ${activeTab === "lesson" ? "active" : ""}`}
          onClick={() => onTabChange("lesson")}
        >
          <GraduationCap className="w-4 h-4" />
          Lesson
          {hasLesson && <span className="learn-tab-dot" />}
        </button>
      </div>
    </div>
  );
};

export default LearnTabs;
