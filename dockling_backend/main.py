from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from docling.document_converter import DocumentConverter
import uvicorn
import spacy
import nltk
from nltk.wsd import lesk
from nltk.corpus import wordnet, stopwords

app = FastAPI()

# Add CORS middleware to allow requests from your Tauri app / frontend
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Allows all origins (or specify "http://localhost:1421" etc.)
    allow_credentials=True,
    allow_methods=["*"],  # Allows all methods (GET, POST, OPTIONS, etc.)
    allow_headers=["*"],  # Allows all headers
)

converter = DocumentConverter()

# Load spaCy model
nlp = spacy.load("en_core_web_sm")

# Load NLTK stopwords
stop_words = set(stopwords.words("english"))

class ConvertRequest(BaseModel):
    path: str

class PromptRequest(BaseModel):
    prompt: str
    max_synonyms_per_word: int = 2

@app.get("/health")
def health():
    return {"status": "ok"}

@app.post("/convert")
def convert_pdf(req: ConvertRequest):
    try:
        result = converter.convert(req.path)
        markdown = result.document.export_to_markdown()
        return {"markdown": markdown}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

def get_wordnet_pos(spacy_pos):
    if spacy_pos.startswith('J'):
        return wordnet.ADJ
    elif spacy_pos.startswith('V'):
        return wordnet.VERB
    elif spacy_pos.startswith('N'):
        return wordnet.NOUN
    elif spacy_pos.startswith('R'):
        return wordnet.ADV
    return None

@app.post("/enhance-prompt")
def enhance_prompt(req: PromptRequest):
    try:
        doc = nlp(req.prompt)
        context_tokens = [t.text for t in doc]
        expansions = []
        seen = set()

        for token in doc:
            # 1. Skip punctuation, spaces, stopwords, short words, numbers
            if (
                token.is_punct
                or token.is_space
                or token.like_num
                or len(token.text) <= 2
                or token.text.lower() in stop_words
            ):
                continue

            # 2. Skip acronyms (e.g., RAG, API, LLM), proper nouns, and named entities
            if token.is_upper or token.pos_ == "PROPN" or token.ent_type_:
                continue

            wn_pos = get_wordnet_pos(token.tag_)
            if not wn_pos:
                continue

            lemma = token.lemma_.lower()
            seen.add(lemma)
            seen.add(token.text.lower())

            # 3. Use Lesk algorithm for context-aware Word Sense Disambiguation
            best_synset = lesk(context_tokens, lemma, pos=wn_pos)
            synsets = [best_synset] if best_synset else wordnet.synsets(lemma, pos=wn_pos)[:1]

            word_synonyms = []
            for synset in synsets:
                if not synset:
                    continue
                for lemma_name in synset.lemma_names():
                    clean_name = lemma_name.replace("_", " ").lower()
                    # Filter out identical words, already-seen terms, and weird hyphenations
                    if clean_name not in seen and "-" not in clean_name:
                        word_synonyms.append(clean_name)
                        seen.add(clean_name)
                    if len(word_synonyms) >= req.max_synonyms_per_word:
                        break
                if len(word_synonyms) >= req.max_synonyms_per_word:
                    break

            expansions.extend(word_synonyms)

        # 4. Append related terms cleanly without mangling the core sentence syntax
        if expansions:
            enhanced_prompt = f"{req.prompt.strip()} (related terms: {', '.join(expansions)})"
        else:
            enhanced_prompt = req.prompt.strip()

        return {
            "original_prompt": req.prompt,
            "enhanced_prompt": enhanced_prompt,
            "keywords": expansions,
        }
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=48484)