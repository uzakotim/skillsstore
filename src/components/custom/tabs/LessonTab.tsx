import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { GraduationCap, Loader2Icon, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

export interface LessonTabProps {
  selectedConcept: string;
  selectedBookId: string;
  isGenerating: boolean;
  lesson: string;
  onGenerateLesson: () => void;
  onBrowseConcepts: () => void;
}

export const LessonTab: React.FC<LessonTabProps> = ({
  selectedConcept,
  selectedBookId,
  isGenerating,
  lesson,
  onGenerateLesson,
  onBrowseConcepts,
}) => {
  return (
    <div className="lesson-tab-content animate-fadein">
      <div className="lesson-workspace">
        <div className="lesson-workspace-header">
          <div className="lesson-topic-title">
            <span className="topic-subtitle">AI LESSON</span>
            <h2>{selectedConcept || "Select a concept"}</h2>
          </div>
        </div>

        {!selectedConcept ? (
          <div className="panel-empty lesson-empty-state">
            <GraduationCap className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />
            <h3>Select a Concept</h3>
            <p>
              Choose a concept from the <strong>Key Concepts</strong> tab to
              generate a lesson.
            </p>
            <Button
              variant="secondary"
              className="mt-4"
              onClick={onBrowseConcepts}
            >
              Browse Key Concepts
            </Button>
          </div>
        ) : isGenerating && !lesson ? (
          <div className="panel-empty lesson-empty-state">
            <Loader2Icon className="w-10 h-10 animate-spin text-primary mb-3" />
            <h3>Generating Lesson...</h3>
            <p>
              Building a lesson about <strong>{selectedConcept}</strong> from your
              book.
            </p>
          </div>
        ) : lesson ? (
          <div className="ai-response-content prose prose-sm dark:prose-invert max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{lesson}</ReactMarkdown>
          </div>
        ) : (
          <div className="panel-empty lesson-empty-state">
            <GraduationCap className="w-10 h-10 text-muted-foreground mb-3 opacity-60" />
            <h3>Ready to Learn</h3>
            <p>
              Generate a lesson about <strong>{selectedConcept}</strong> using the
              selected book.
            </p>
            <Button
              className="mt-4"
              onClick={onGenerateLesson}
              disabled={!selectedBookId}
            >
              <Sparkles className="w-4 h-4 mr-2" />
              Generate Lesson
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};

export default LessonTab;
