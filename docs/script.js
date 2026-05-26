document.documentElement.classList.add('js');

const header = document.querySelector('.site-header');
const revealTargets = document.querySelectorAll('[data-reveal]');

const updateHeader = () => {
  if (!header) return;
  header.dataset.scrolled = String(window.scrollY > 18);
};

const revealFallback = () => {
  for (const target of revealTargets) {
    target.classList.add('is-visible');
  }
};

const setupReveal = () => {
  if (!revealTargets.length) return;

  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches || !('IntersectionObserver' in window)) {
    revealFallback();
    return;
  }

  for (const target of revealTargets) {
    const rect = target.getBoundingClientRect();
    if (rect.top < window.innerHeight * 1.15 && rect.bottom > 0) {
      target.classList.add('is-visible');
    }
  }

  const observer = new IntersectionObserver(
    (entries, obs) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add('is-visible');
        obs.unobserve(entry.target);
      }
    },
    { threshold: 0.18 }
  );

  for (const target of revealTargets) {
    observer.observe(target);
  }
};

updateHeader();
setupReveal();
window.addEventListener('scroll', updateHeader, { passive: true });
