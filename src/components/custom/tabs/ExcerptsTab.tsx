import React from "react";
import { Loader2Icon, GraduationCap, BookOpen } from "lucide-react";
import { Button } from "@/components/ui/button";

export interface ExcerptsTabProps {
  selectedConcept: string;
  selectedBookId: string;
  isGenerating: boolean;
  relatedExcerpts: string[];
  onGenerateLesson: () => void;
  onBrowseConcepts: () => void;
}

export const ExcerptsTab: React.FC<ExcerptsTabProps> = ({
  selectedConcept,
  selectedBookId,
  isGenerating,
  relatedExcerpts,
  onGenerateLesson,
  onBrowseConcepts,
}) => {
  return (
    <div className="lesson-tab-content animate-fadein">
      <div className="lesson-workspace">
        <div className="lesson-workspace-header">
          <div className="lesson-topic-title">
            {selectedConcept ? (
              <>
                <span className="topic-subtitle">RELATED EXCERPTS</span>
                <h2>{selectedConcept}</h2>
              </>
            ) : (
              <h2>Related Book Excerpts</h2>
            )}
          </div>

          <Button
            onClick={onGenerateLesson}
            disabled={
              isGenerating ||
              !selectedBookId ||
              !selectedConcept.trim()
            }
          >
            {isGenerating ? (
              <>
                <Loader2Icon className="w-4 h-4 mr-2 animate-spin" />
                Generating...
              </>
            ) : (
              <>
                <GraduationCap className="w-4 h-4 mr-2" />
                Generate Lesson
              </>
            )}
          </Button>
        </div>

        {relatedExcerpts.length > 0 ? (
          <div className="context-list">
            {relatedExcerpts.map((excerpt, i) => (
              <div key={i} className="context-item">
                <span className="context-num">{i + 1}</span>
                <p className="context-text">"{excerpt}"</p>
              </div>
            ))}
          </div>
        ) : (
          <div className="panel-empty lesson-empty-state">
            {isGenerating ? (
              <>
                <Loader2Icon className="w-10 h-10 animate-spin text-primary mb-3" />
                <h3>Finding Related Excerpts...</h3>
                <p>
                  Searching the selected book for passages related to this concept.
                </p>
              </>
            ) : (
              <>
                <BookOpen className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />
                <h3>No Related Excerpts</h3>
                <p>
                  Select a concept from the <strong>Key Concepts</strong> tab or
                  type a topic to find matching passages in the book.
                </p>
                <Button
                  variant="secondary"
                  className="mt-4"
                  onClick={onBrowseConcepts}
                >
                  Browse Key Concepts
                </Button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default ExcerptsTab;
