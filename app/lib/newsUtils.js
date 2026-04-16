import Script from 'next/script'

function Post(href) {
  return (
    <>
      <div id="fb-root"></div>
      <Script 
        src={href}
        strategy="lazyOnload" 
      />
    </>
  )
}

export function parseHTMLDescription(description) {
    const parser = new DOMParser();
    const htmlDoc = parser.parseFromString(description, 'text/html');
    const ps = [...htmlDoc.getElementsByTagName('p')];
    ps.forEach(p => p.classList.add("new-description-p"));
    const iframes = [...htmlDoc.getElementsByTagName('iframe')];

    const links = [...htmlDoc.getElementsByTagName('a')];
    links.forEach(link => {
        const href = link.getAttribute('href') || '';
        if (href.includes('twitter.com') || href.includes('x.com') || 
            href.includes('facebook.com') || href.includes('instagram.com')) {
                const a = 2;
        }
    });
    
    iframes.forEach(iframe => {
        iframe.height = "";
        iframe.width = "";
        iframe.classList.add("new-description-embedded-iframe");
    });

    return htmlDoc.body.innerHTML;
}