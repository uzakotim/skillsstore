import React from "react";
import { GraduationCap, Loader2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { LearnTabs, ConceptsTab, ExcerptsTab, LessonTab } from "./tabs";
import { AnalysisProgress, ConceptItem, LearnTab } from "@/types";

export interface LearnModeProps {
  selectedBookId: string;
  isGenerating: boolean;
  analysisProgress: AnalysisProgress | null;
  learningPath: string;
  parsedConcepts: ConceptItem[];
  learnTab: LearnTab;
  onTabChange: (tab: LearnTab) => void;
  selectedConcept: string;
  onSelectConcept: (concept: string) => void;
  relatedExcerpts: string[];
  onSetRelatedExcerpts: (excerpts: string[]) => void;
  lesson: string;
  onGenerateLearningPath: () => void;
  onGetRelatedExcerpts: (concept: string) => void;
  onGenerateLesson: () => void;
}

export const LearnMode: React.FC<LearnModeProps> = ({
  selectedBookId,
  isGenerating,
  analysisProgress,
  learningPath,
  parsedConcepts,
  learnTab,
  onTabChange,
  selectedConcept,
  onSelectConcept,
  relatedExcerpts,
  onSetRelatedExcerpts,
  lesson,
  onGenerateLearningPath,
  onGetRelatedExcerpts,
  onGenerateLesson,
}) => {
  return (
    <div className="learn-mode animate-fadein">
      {!learningPath && !isGenerating && (
        <div className="learn-start">
          <div className="learn-icon">
            <GraduationCap className="w-8 h-8" />
          </div>
          <h2>Start Learning</h2>
          <p>
            Select a book from your shelf, then let AI identify the key concepts
            and show the relevant passages from the book.
          </p>
          <Button
            onClick={onGenerateLearningPath}
            size="lg"
            disabled={!selectedBookId}
            className="learn-start-btn"
          >
            {selectedBookId ? "Find Key Concepts" : "Select a book first"}
          </Button>
          {!selectedBookId && (
            <p className="learn-hint">
              ↑ Click a book on the shelf above to get started
            </p>
          )}
        </div>
      )}

      {isGenerating && !learningPath && (
        <div className="learn-loading">
          <Loader2Icon className="w-12 h-12 text-primary animate-spin" />

          <p>
            {analysisProgress?.status ||
              "Analysing your book's key concepts..."}
          </p>

          {analysisProgress?.percent != null && (
            <div className="pull-progress-bar rounded-xl w-[80%]">
              <div className="pull-info flex gap-10">
                <span className="pull-status">Progress</span>
                <span className="pull-percent">
                  {analysisProgress.percent.toFixed(0)}%
                </span>
              </div>

              <div className="pull-track">
                <div
                  className="pull-fill"
                  style={{
                    width: `${analysisProgress.percent}%`,
                    transition: "width 0.3s ease",
                  }}
                />
              </div>

              {analysisProgress.processed != null &&
                analysisProgress.total != null && (
                  <div className="pull-info">
                    <span className="pull-status">
                      {analysisProgress.processed.toLocaleString()} /{" "}
                      {analysisProgress.total.toLocaleString()} tokens
                    </span>
                  </div>
                )}
            </div>
          )}
        </div>
      )}

      {learningPath && (
        <div className="learn-container">
          <LearnTabs
            activeTab={learnTab}
            onTabChange={onTabChange}
            conceptCount={parsedConcepts.length}
            hasSelectedConcept={Boolean(selectedConcept)}
            hasLesson={Boolean(lesson)}
          />

          {learnTab === "concepts" && (
            <ConceptsTab
              learningPath={learningPath}
              parsedConcepts={parsedConcepts}
              selectedConcept={selectedConcept}
              onSelectConcept={(conceptTitle, excerpts) => {
                onSelectConcept(conceptTitle);
                onSetRelatedExcerpts(excerpts);
                onTabChange("excerpts");
              }}
              onFindRelatedExcerpts={onGetRelatedExcerpts}
              isGenerating={isGenerating}
            />
          )}

          {learnTab === "excerpts" && (
            <ExcerptsTab
              selectedConcept={selectedConcept}
              selectedBookId={selectedBookId}
              isGenerating={isGenerating}
              relatedExcerpts={relatedExcerpts}
              onGenerateLesson={onGenerateLesson}
              onBrowseConcepts={() => onTabChange("concepts")}
            />
          )}

          {learnTab === "lesson" && (
            <LessonTab
              selectedConcept={selectedConcept}
              selectedBookId={selectedBookId}
              isGenerating={isGenerating}
              lesson={lesson}
              onGenerateLesson={onGenerateLesson}
              onBrowseConcepts={() => onTabChange("concepts")}
            />
          )}
        </div>
      )}
    </div>
  );
};

export default LearnMode;
