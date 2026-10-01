// Conveniences only. Every page works without this file.
document.querySelectorAll('[data-copy]').forEach(function (button) {
  var source = document.getElementById(button.getAttribute('data-copy'));
  if (!source || !navigator.clipboard) return;
  button.hidden = false;
  button.addEventListener('click', function () {
    navigator.clipboard.writeText(source.textContent.trim()).then(function () {
      button.textContent = 'Copied';
    });
  });
});
document.querySelectorAll('[data-print]').forEach(function (button) {
  button.hidden = false;
  button.addEventListener('click', function () {
    window.print();
  });
});
