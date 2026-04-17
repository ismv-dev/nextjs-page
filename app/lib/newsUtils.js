function socialEmbed(href) {
  // implementar presentacion de post de red social
  return href;
}

export function parseHTMLDescription(description) {
    const parser = new DOMParser();
    const htmlDoc = parser.parseFromString(description, 'text/html');
    const ps = [...htmlDoc.getElementsByTagName('p')];
    ps.forEach(p => p.classList.add("new-description-p"));
    const iframes = [...htmlDoc.getElementsByTagName('iframe')];
    const imgs = [...htmlDoc.getElementsByTagName('img')];

    const links = [...htmlDoc.getElementsByTagName('a')];
    links.forEach(link => {
        const href = link.getAttribute('href') || '';
        if (href.includes('twitter.com') || href.includes('x.com') || 
            href.includes('facebook.com') || href.includes('instagram.com')) {
              socialEmbed(href);
        }
    });
    
    iframes.forEach(iframe => {
        iframe.height = "";
        iframe.width = "";
        iframe.classList.add("new-description-embedded-iframe");
    });
    
    imgs.forEach(iframe => {
        iframe.height = "";
        iframe.width = "";
    });

    return htmlDoc.body.innerHTML;
}