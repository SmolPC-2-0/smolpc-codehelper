# Phase 4: Educational Features & Multi-User

**Goal:** Add educational features, multi-user support, and teacher administration.

**Prerequisites:**
- Phase 1-3 complete (core engine with hardware acceleration)

---

## Objectives

1. Implement multi-user profiles for shared computers
2. Add response style selector (complexity dial)
3. Enable chat export to Markdown
4. Build basic teacher administration panel
5. Add system prompt customization
6. Implement student activity logging
7. Add progress report export

---

## Deliverables

### Multi-User Profiles
- [ ] Profile creation/selection on startup
- [ ] Profile-isolated chat storage
- [ ] Profile-specific settings
- [ ] Profile switching
- [ ] Profile deletion with data cleanup

### Response Style (Complexity Dial)
- [ ] Simple mode (ages 11-13)
- [ ] Standard mode (ages 14-16)
- [ ] Detailed mode (ages 17-18)
- [ ] System prompt modification per mode

### Teacher Features
- [ ] Teacher admin login
- [ ] View student activity summary
- [ ] Custom system prompt configuration
- [ ] Lesson context setting
- [ ] Progress report generation

### Data Management
- [ ] Chat export to Markdown
- [ ] Clear all user data
- [ ] Activity logging (local only)
- [ ] Progress report export (JSON/PDF)

---

## Profile System

### Storage Structure

```
~/.smolpc/
├── profiles/
│   ├── default/
│   │   ├── profile.json
│   │   ├── chats/
│   │   └── settings.json
│   ├── alex/
│   │   ├── profile.json
│   │   ├── chats/
│   │   └── settings.json
│   └── sarah/
│       └── ...
└── admin/
    ├── teacher_config.json
    └── activity_logs/
```

### Profile Schema

```typescript
interface Profile {
  id: string;
  name: string;
  createdAt: string;
  settings: {
    responseStyle: 'simple' | 'standard' | 'detailed';
    systemPrompt?: string;
  };
}
```

---

## Response Style System

### System Prompt Templates

**Simple (11-13):**
```
You are a friendly coding helper for young students.
- Use very simple words
- Give short explanations
- Use fun analogies (like comparing loops to repeating songs)
- Always be encouraging
- Break everything into tiny steps
```

**Standard (14-16):**
```
You are a coding tutor for secondary school students.
- Explain concepts clearly
- Use proper programming terms but define them
- Give balanced explanations with examples
- Encourage good practices
```

**Detailed (17-18):**
```
You are a coding assistant for advanced students.
- Be concise and technical
- Assume basic programming knowledge
- Provide detailed explanations when asked
- Discuss trade-offs and best practices
```

---

## Teacher Administration

### Admin Panel Features

1. **Activity Overview** - Which students used the app, how many questions
2. **Topic Analysis** - What topics students are asking about
3. **System Prompt Override** - Set class-wide prompt modifications
4. **Lesson Context** - Set current lesson topic for context-aware responses
5. **Progress Reports** - Export student usage data

### Activity Logging

Log locally, never transmit:

```typescript
interface ActivityLog {
  profileId: string;
  timestamp: string;
  action: 'question' | 'response';
  topics: string[];  // Extracted keywords
  messageLength: number;
  responseTime: number;
}
```

---

## Success Criteria

| Criteria | Target |
|----------|--------|
| Multiple profiles work | Yes |
| Response styles differ noticeably | Yes |
| Chat export works | Yes |
| Teacher can view activity | Yes |
| Data stays local | Yes |

---

*When Phase 4 is complete, proceed to PHASE-5.md for SmolPC Launcher ecosystem.*
