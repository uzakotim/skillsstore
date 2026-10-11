import React, { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArrowRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Modal, ModalHeader, ModalBody, ModalTitle } from "@/components/custom/Modal";
import { ConceptItem } from "@/types";

export interface ConceptsTabProps {
  learningPath: string;
  parsedConcepts: ConceptItem[];
  selectedConcept: string;
  onSelectConcept: (conceptTitle: string, excerpts: string[]) => void;
  onFindRelatedExcerpts: (concept: string) => void;
  isGenerating: boolean;
}

export const ConceptsTab: React.FC<ConceptsTabProps> = ({
  learningPath,
  parsedConcepts,
  selectedConcept,
  onSelectConcept,
  onFindRelatedExcerpts,
  isGenerating,
}) => {
  const [customConcept, setCustomConcept] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const [modalTitle, setModalTitle] = useState("");
  const [modalDescription, setModalDescription] = useState("");

  const handleCustomSearch = () => {
    if (customConcept.trim()) {
      onFindRelatedExcerpts(customConcept);
    }
  };

  return (
    <div className="concepts-tab-content animate-fadein">
      {/* Concept search & custom prompt box */}
      <div className="custom-concept-bar">
        <input
          type="text"
          placeholder="Or type a specific concept/topic to find in the book..."
          className="custom-concept-input"
          value={customConcept}
          onChange={(e) => setCustomConcept(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleCustomSearch()}
        />
        <Button
          onClick={handleCustomSearch}
          disabled={isGenerating || !customConcept.trim()}
          size="sm"
        >
          Find Related Excerpts
        </Button>
      </div>

      {parsedConcepts.length > 0 ? (
        <div className="concepts-grid">
          {parsedConcepts.map((item, idx) => (
            <div
              key={item.id || idx}
              className={`concept-card ${
                selectedConcept === item.title ? "active" : ""
              }`}
              onClick={() => {
                onSelectConcept(item.title, item.excerpts || []);
              }}
            >
              <div className="concept-card-header">
                <span className="concept-number">{idx + 1}</span>
                <h3 className="concept-card-title">{item.title}</h3>
              </div>
              {item.description && (
                <p className="concept-card-desc">{item.description}</p>
              )}
              <div className="concept-card-footer">
                <span
                  className="concept-tag concept-tag-clickable"
                  onClick={(e) => {
                    e.stopPropagation();
                    setModalTitle(item.title);
                    setModalDescription(item.description || "");
                    setModalOpen(true);
                  }}
                >
                  Concept
                </span>
                <span className="concept-action">
                  View Excerpts{" "}
                  <ArrowRight className="w-3.5 h-3.5 ml-1 inline" />
                </span>
              </div>
            </div>
          ))}
        </div>
      ) : (
        /* Fallback Markdown View if concept parsing finds no clear items */
        <div className="concepts-fallback-panel">
          <div className="prose prose-sm dark:prose-invert max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {learningPath}
            </ReactMarkdown>
          </div>
        </div>
      )}

      {/* Modal for viewing concept description */}
      <Modal isOpen={modalOpen} onClose={() => setModalOpen(false)}>
        <ModalHeader onClose={() => setModalOpen(false)}>
          <ModalTitle>{modalTitle}</ModalTitle>
        </ModalHeader>
        <ModalBody>
          <ReactMarkdown remarkPlugins={[remarkGfm]}>
            {modalDescription}
          </ReactMarkdown>
        </ModalBody>
      </Modal>
    </div>
  );
};

export default ConceptsTab;
