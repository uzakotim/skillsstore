from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from docling.document_converter import DocumentConverter
import uvicorn

app = FastAPI()
converter = DocumentConverter()


class ConvertRequest(BaseModel):
    path: str


@app.get("/health")
def health():
    return {"status": "ok"}


@app.post("/convert")
def convert_pdf(req: ConvertRequest):
    try:
        result = converter.convert(req.path)
        markdown = result.document.export_to_markdown()
        return {"markdown": markdown}
    except FileNotFoundError as e:
        raise HTTPException(status_code=400, detail=f"File not found: {e}")
    except ValueError as e:
        raise HTTPException(status_code=400, detail=f"Invalid input: {e}")
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=48484)
