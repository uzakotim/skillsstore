import { Button } from "@/components/ui/button";
import { X } from "lucide-react";


export const Modal = ({ isOpen, onClose, children }: { isOpen: boolean; onClose: () => void; children: React.ReactNode }) => {
    if (!isOpen) return null;
    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
            <div className="absolute inset-0 bg-black/50" onClick={onClose} />
            <div className=" z-50 bg-white p-10 rounded-lg shadow-lg max-w-[60%] my-auto">
                {children}
            </div>
        </div>
    );
};

export const ModalHeader = ({ children, onClose }: { children: React.ReactNode; onClose: () => void }) => {
    return <div className="flex items-center justify-between concept-card-title" >
        {children}
        <Button variant="ghost" onClick={() => onClose()}>
            <X className="w-4 h-4" />
        </Button>
    </div>;
};

export const ModalBody = ({ children }: { children: React.ReactNode }) => {
    return <div className="mt-8 text-gray-500 flex-1 text-sm text-justify">{children}</div>;
};

export const ModalTitle = ({ children }: { children: React.ReactNode }) => {
    return <div className="mt-2">{children}</div>;
}