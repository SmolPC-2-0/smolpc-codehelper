// Tauri v2 API
const invoke = window.__TAURI_INTERNALS__.invoke;

// Use example prompt
function useExample(prompt) {
    document.getElementById('question').value = prompt;
    document.getElementById('codeInput').value = ''; // Clear code input
}

// Generate code
async function generateCode() {
    const question = document.getElementById('question').value.trim();
    const codeInput = document.getElementById('codeInput').value.trim();
    const model = document.getElementById('modelSelect').value;
    const output = document.getElementById('codeOutput');
    const loading = document.getElementById('loading');
    const status = document.getElementById('status');
    const generateBtn = document.getElementById('generateBtn');

    // Validation
    if (!question && !codeInput) {
        showStatus('Please enter a question or paste code to debug', 'error');
        return;
    }

    // Build prompt
    let prompt = '';
    if (codeInput) {
        prompt = `The student has this code:\n\`\`\`\n${codeInput}\n\`\`\`\n\nTheir question: ${question || 'What is wrong with this code and how can I fix it?'}`;
    } else {
        prompt = question;
    }

    // Show loading
    loading.classList.remove('hidden');
    generateBtn.disabled = true;
    output.textContent = '';
    showStatus('Generating...', '');

    try {
        const result = await invoke('generate_code', { 
            prompt: prompt,
            model: model 
        });
        
        output.textContent = result;
        showStatus('Code generated successfully!', 'success');
        // Auto-clear inputs after successful generation
        document.getElementById('question').value = '';
        document.getElementById('codeInput').value = '';
    } catch (error) {
        output.textContent = `Error: ${error}`;
        showStatus(`Error: ${error}`, 'error');
    } finally {
        loading.classList.add('hidden');
        generateBtn.disabled = false;
    }
}

// Copy code to clipboard
async function copyCode() {
    const code = document.getElementById('codeOutput').textContent;
    
    if (code === 'Your generated code will appear here...') {
        showStatus('No code to copy', 'error');
        return;
    }

    try {
        await navigator.clipboard.writeText(code);
        showStatus('Code copied to clipboard!', 'success');
    } catch (error) {
        showStatus('Failed to copy code', 'error');
    }
}

// Save code to file
async function saveCode() {
    const code = document.getElementById('codeOutput').textContent;
    
    if (code === 'Your generated code will appear here...') {
        showStatus('No code to save', 'error');
        return;
    }

    try {
        await invoke('save_code', { content: code });
        showStatus('Code saved successfully!', 'success');
    } catch (error) {
        showStatus(`Failed to save: ${error}`, 'error');
    }
}

// Clear all inputs and outputs
function clearAll() {
    document.getElementById('question').value = '';
    document.getElementById('codeInput').value = '';
    document.getElementById('codeOutput').textContent = 'Your generated code will appear here...';
    showStatus('Cleared', '');
}

// Show status message
function showStatus(message, type) {
    const status = document.getElementById('status');
    status.textContent = message;
    status.className = 'status';
    if (type) status.classList.add(type);
}

// Allow Enter key to submit (with Ctrl/Cmd)
document.getElementById('question').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        generateCode();
    }
});

// Check Ollama on startup
window.addEventListener('DOMContentLoaded', async () => {
    try {
        await invoke('check_ollama');
        showStatus('Ready', 'success');
    } catch (error) {
        showStatus('Ollama not running! Please start Ollama first.', 'error');
    }
});