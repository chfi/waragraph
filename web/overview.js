

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

        ctx.strokeStyle = "red";
        ctx.lineWidth = 2;

        ctx.beginPath();

        ctx.rect(x0, -5.0, x1 - x0, this.canvas.height + 10.0);

        ctx.stroke();

        this.latest_view = view;
    }
}


export { OverviewMap };
