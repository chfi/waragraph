

class OverviewMap {
    constructor(coord_sys, canvas) {
        this.coord_sys = coord_sys;
        this.canvas = canvas;
        this.latest_view = null;
    }

    draw(view) {
        const ctx = this.canvas.getContext('2d');

        console.log("drawing overview");

        let c_width = this.canvas.width;
        let max = view.max;

        let x0 = (view.left / max) * c_width;
        let x1 = (view.right / max) * c_width;

        console.log('left: ' + view.left + ', right: ' + view.right + ', max: ' + view.max);
        console.log('x0: ' + x0 + ', x1: ' + x1);

        ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

        
        let left_txt = Math.round(view.left).toString();
        let right_txt = Math.round(view.right).toString();

        let left_w = ctx.measureText(left_txt).width;
        let right_w = ctx.measureText(right_txt).width;

        let between_len = x1 - x0;

        let y0 = this.canvas.height / 2;

        console.log("left_txt width: " + left_w);
        console.log("right_txt width: " + right_w);
        
        ctx.save();
        if (x0 > left_w) {
            ctx.textAlign = "end";
            ctx.fillText(left_txt, x0 - 2.0, y0);
        } else if (between_len > left_w) {
            ctx.textAlign = "start";
            ctx.fillText(left_txt, x0 + 2.0, y0);
        } else {
            ctx.textAlign = "start";
            ctx.fillText(left_txt, x1 + 2.0, y0);
        }
        ctx.restore();

        if (c_width - x1 > right_w) {
            ctx.textAlign = "start";
            ctx.fillText(right_txt, x1 + 2.0, y0);
        } else if (between_len > right_w) {
            ctx.textAlign = "end";
            ctx.fillText(right_txt, x1 - 2.0, y0);
        } else {
            ctx.textAlign = "start";
            ctx.fillText(right_txt, x1 + 2.0, y0);
        }

        ctx.strokeStyle = "red";
        ctx.lineWidth = 2;

        ctx.beginPath();

        ctx.rect(x0, -5.0, x1 - x0, this.canvas.height + 10.0);

        ctx.stroke();

        this.latest_view = view;
    }
}


export { OverviewMap };
