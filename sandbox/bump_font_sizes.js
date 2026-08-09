const fs = require('fs');
const path = require('path');

const srcDir = path.resolve(__dirname, '../app/src');

function getAllFiles(dir, fileList = []) {
  const files = fs.readdirSync(dir);
  for (const file of files) {
    const filePath = path.join(dir, file);
    if (fs.statSync(filePath).isDirectory()) {
      getAllFiles(filePath, fileList);
    } else if (/\.(tsx|ts|jsx|js|css)$/.test(file)) {
      fileList.push(filePath);
    }
  }
  return fileList;
}

const files = getAllFiles(srcDir);
let totalReplacements = 0;
let modifiedFiles = 0;

for (const filePath of files) {
  let content = fs.readFileSync(filePath, 'utf8');
  const original = content;

  // Replace text-[Xpx]
  content = content.replace(/text-\[(\d+(?:\.\d+)?)px\]/g, (match, valStr) => {
    const val = parseFloat(valStr);
    let newVal;
    if (val < 9) {
      newVal = 9;
    } else {
      newVal = val + 1;
    }
    
    // Format nicely (e.g. 10.5 vs 11)
    const newValStr = Number.isInteger(newVal) ? `${newVal}` : `${newVal}`;
    const result = `text-[${newValStr}px]`;
    if (result !== match) {
      totalReplacements++;
    }
    return result;
  });

  if (content !== original) {
    fs.writeFileSync(filePath, content, 'utf8');
    modifiedFiles++;
    console.log(`Updated: ${path.relative(srcDir, filePath)}`);
  }
}

console.log(`\nFinished! Modified ${modifiedFiles} files with ${totalReplacements} text-size replacements.`);
