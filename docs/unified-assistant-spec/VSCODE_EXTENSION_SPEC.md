# VS Code Extension Spec

**Last Updated:** 2026-03-16
**Status:** Historical / future work, not active for the unified implementation

## 1. Scope Change

This document is **not** part of the active unified frontend plan.

The current unified product direction is:

- one Tauri app
- Code as an in-app mode
- no separate VS Code extension in the active implementation sequence

## 2. Why This Is Out Of Scope

The immediate goal is to preserve current Codehelper behavior inside the unified
desktop app while GIMP, Blender, Writer, Calc, and Slides are added as peer
modes.

That means a separate VS Code extension is no longer the implementation path for
Code mode in this workstream.

## 3. How To Treat This File

Use this file only as:

- historical context for a prior architecture direction, or
- future-work notes if editor-native integration is reconsidered later

Do **not** treat it as active guidance for the unified frontend.

## 4. Revisit Criteria

Only reopen extension work if a future decision explicitly changes one of these:

- Code mode should leave the unified app
- editor-native integrations become a required delivery target
- product requirements expand beyond the current unified desktop scope
