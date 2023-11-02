// import {computePosition} from 'https://cdn.jsdelivr.net/npm/@floating-ui/dom@1.5.3/+esm';
import {computePosition} from '@floating-ui/dom';


export function placeTooltipAtPoint(x, y) {
    const tooltip = document.getElementById('tooltip');

    const virtualEl = {
        getBoundingClientRect() {
            return {
                width: 0,
                height: 0,
                x,
                y,
                top: y,
                bottom: y,
                left: x,
                right: x,
            };
        },
    };


    computePosition(virtualEl, tooltip).then(({x, y}) => {
        Object.assign(tooltip.style, {
            left: `${x}px`,
            top: `${y}px`,
        });
    });
}
