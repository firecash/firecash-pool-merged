"use client";

import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { Menu, X } from "lucide-react";
import { Brand } from "./brand";
import { SidebarNav } from "./sidebar-nav";
import { Button } from "@/components/ui/button";

/** Hamburger that opens a slide-over nav drawer on small screens. */
export function MobileNav() {
  const [open, setOpen] = useState(false);

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <Button variant="ghost" size="icon" className="lg:hidden" aria-label="Open menu">
          <Menu className="size-5" />
        </Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <Dialog.Content className="dark fixed inset-y-0 left-0 z-50 flex w-72 flex-col border-r border-border bg-brand-bg p-4 text-foreground shadow-xl data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left">
          <div className="flex items-center justify-between">
            <Brand className="px-1.5" />
            <Dialog.Close asChild>
              <Button variant="ghost" size="icon" aria-label="Close menu">
                <X className="size-5" />
              </Button>
            </Dialog.Close>
          </div>
          <Dialog.Title className="sr-only">Navigation</Dialog.Title>
          <div className="mt-6">
            <SidebarNav onNavigate={() => setOpen(false)} />
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
