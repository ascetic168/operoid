import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** 合併 Tailwind class 並去除衝突（shadcn 慣例）。 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
