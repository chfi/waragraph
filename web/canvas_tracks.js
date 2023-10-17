
export function drawVariableThicknessTrack(canvas, x_ranges) {
    const ctx = canvas.getContext('2d');
    ctx.save();

    console.log(canvas.width);
    console.log(canvas.height);
    const y_mid = canvas.height / 2;

    for (const entry of x_ranges) {
        const { start, end, thickness } = entry;
        ctx.fillRect(start, y_mid - thickness * 0.5, end - start, thickness);
    }

    ctx.restore();
}

