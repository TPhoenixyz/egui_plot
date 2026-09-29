use core::hash::Hash;
use std::ops::RangeInclusive;

use egui::Color32;
use egui::CornerRadius;

use egui::Response;
use egui::Shape;
use egui::Stroke;
use egui::StrokeKind;
use egui::Ui;
use egui::epaint::RectShape;

use emath::Pos2;

use crate::aesthetics::Orientation;
use crate::axis::PlotTransform;
use crate::bounds::PlotBounds;
use crate::bounds::PlotPoint;
use crate::colors::highlighted_color;
use crate::cursor::Cursor;
use crate::item_id::ItemId;
use crate::items::ClosestElem;
use crate::items::PlotConfig;
use crate::items::PlotGeometry;
use crate::items::PlotItem;
use crate::items::PlotItemBase;
use crate::items::add_rulers_and_text;
use crate::label::LabelFormatterFn;
use crate::math::find_closest_rect;
use crate::rect_elem::RectElement;

/// Similar to [`BarChart`] and [`BoxPlot`] but as Candlesticks
pub struct CandlestickChart {
    base: PlotItemBase,

    pub(crate) sticks: Vec<Candlestick>,
    default_color: Color32,

    /// A custom element formatter
    pub(crate) element_formatter: Option<Box<dyn Fn(&Candlestick, &Self) -> String>>,
}

impl CandlestickChart {
    // Create a candlestick chart. It defaults to vertically oriented elements.
    pub fn new(name: impl Into<String>, sticks: Vec<Candlestick>) -> Self {
        Self {
            base: PlotItemBase::new(name.into()),
            sticks,
            default_color: Color32::TRANSPARENT,
            element_formatter: None,
        }
    }

    /// Set the default colors. They are set on all elements that do not already
    /// have a specific color. The body color is the color that shows up in the
    /// legend. It can be overridden at the candlestick level (see [`Candlestick`]).
    /// Default is `Color32::TRANSPARENT` which means a color will be
    /// auto-assigned.
    #[inline]
    pub fn color(mut self, body_color: impl Into<Color32>, whick_color: impl Into<Color32>) -> Self {
        let body_color: Color32 = body_color.into();
        let whick_color: Color32 = whick_color.into();

        let plot_color: Color32 = body_color;
        self.default_color = plot_color;

        let body_fill = body_color.linear_multiply(0.2);
        let whick_fill = whick_color.linear_multiply(0.2);

        for stick in &mut self.sticks {
            if stick.body_fill == Color32::TRANSPARENT && stick.body_stroke.color == Color32::TRANSPARENT {
                stick.body_fill = body_fill;
                stick.body_stroke.color = body_color;
            }
            if stick.whick_fill == Color32::TRANSPARENT && stick.whick_stroke.color == Color32::TRANSPARENT {
                stick.whick_fill = whick_fill;
                stick.whick_stroke.color = whick_color;
            }
        }
        self
    }

    /// Set the default body color. It is set on all elements that do not already
    /// have a specific color. This is the color that shows up in the
    /// legend. It can be overridden at the candlestick level (see [`Candlestick`]).
    /// Default is `Color32::TRANSPARENT` which means a color will be
    /// auto-assigned.
    #[inline]
    pub fn color_body(mut self, color: impl Into<Color32>) -> Self {
        let plot_color: Color32 = color.into();
        self.default_color = plot_color;

        let fill_color = plot_color.linear_multiply(0.2);

        for stick in &mut self.sticks {
            if stick.body_fill == Color32::TRANSPARENT && stick.body_stroke.color == Color32::TRANSPARENT {
                stick.body_fill = fill_color;
                stick.body_stroke.color = plot_color;
            }
        }
        self
    }

    /// Set the default whick color. It is set on all elements that do not already
    /// have a specific color.
    /// Default is `Color32::TRANSPARENT` which means a color will be
    /// auto-assigned.
    #[inline]
    pub fn color_whick(mut self, color: impl Into<Color32>) -> Self {
        let plot_color: Color32 = color.into();

        let fill_color = plot_color.linear_multiply(0.2);

        for stick in &mut self.sticks {
            if stick.whick_fill == Color32::TRANSPARENT && stick.whick_stroke.color == Color32::TRANSPARENT {
                stick.whick_fill = fill_color;
                stick.whick_stroke.color = plot_color;
            }
        }
        self
    }

    /// Set all elements to be in a vertical orientation.
    /// Argument axis will be X and values will be on the Y axis.
    #[inline]
    pub fn vertical(mut self) -> Self {
        for stick in &mut self.sticks {
            stick.orientation = Orientation::Vertical;
        }
        self
    }

    /// Set all elements to be in a horizontal orientation.
    /// Argument axis will be Y and values will be on the X axis.
    #[inline]
    pub fn horizontal(mut self) -> Self {
        for stick in &mut self.sticks {
            stick.orientation = Orientation::Horizontal;
        }
        self
    }

    /// Set the width (thickness) of all its elements.
    #[inline]
    pub fn width(mut self, body_width: f64, whick_width: f64) -> Self {
        for b in &mut self.sticks {
            b.body_width = body_width;
            b.whick_width = whick_width;
        }
        self
    }

    /// Set the width (thickness) of all its body elements.
    #[inline]
    pub fn width_body(mut self, width: f64) -> Self {
        for b in &mut self.sticks {
            b.body_width = width;
        }
        self
    }

    /// Set the width (thickness) of all its whick elements.
    #[inline]
    pub fn width_whick(mut self, width: f64) -> Self {
        for b in &mut self.sticks {
            b.whick_width = width;
        }
        self
    }

    /// Add a custom way to format an element.
    /// Can be used to display a set number of decimals or custom labels.
    #[inline]
    pub fn element_formatter(mut self, formatter: Box<dyn Fn(&Candlestick, &Self) -> String>) -> Self {
        self.element_formatter = Some(formatter);
        self
    }

    /// Name of this plot item.
    ///
    /// This name will show up in the plot legend, if legends are turned on.
    ///
    /// Setting the name via this method does not change the item's id, so you
    /// can use it to change the name dynamically between frames without
    /// losing the item's state. You should make sure the name passed to
    /// [`Self::new`] is unique and stable for each item, or set unique and
    /// stable ids explicitly via [`Self::id`].
    #[expect(clippy::needless_pass_by_value, reason = "to allow various string types")]
    #[inline]
    pub fn name(mut self, name: impl ToString) -> Self {
        self.base_mut().name = name.to_string();
        self
    }

    /// Highlight this plot item, typically by scaling it up.
    ///
    /// If false, the item may still be highlighted via user interaction.
    #[inline]
    pub fn highlight(mut self, highlight: bool) -> Self {
        self.base_mut().highlight = highlight;
        self
    }

    /// Allowed hovering this item in the plot. Default: `true`.
    #[inline]
    pub fn allow_hover(mut self, hovering: bool) -> Self {
        self.base_mut().allow_hover = hovering;
        self
    }

    /// Sets the [`ItemId`] of this plot item.
    ///
    /// The id only has to be unique within the plot.
    ///
    /// By default the id is derived from the name passed to [`Self::new`],
    /// but it can be explicitly set to a different value.
    #[inline]
    pub fn id(mut self, id: impl Hash) -> Self {
        self.base_mut().id = ItemId::new(id);
        self
    }
}
impl PlotItem for CandlestickChart {
    fn shapes(&self, _ui: &Ui, transform: &PlotTransform, shapes: &mut Vec<Shape>) {
        for stick in &self.sticks {
            stick.add_shapes(transform, self.base.highlight, shapes);
        }
    }

    fn initialize(&mut self, _x_range: RangeInclusive<f64>) {
        // nothing to do
    }

    fn color(&self) -> Color32 {
        self.default_color
    }

    fn geometry(&self) -> PlotGeometry<'_> {
        PlotGeometry::Rects
    }

    fn bounds(&self) -> PlotBounds {
        let mut bounds = PlotBounds::NOTHING;
        for stick in &self.sticks {
            bounds.merge(&stick.bounds());
        }
        bounds
    }

    fn find_closest(&self, point: Pos2, transform: &PlotTransform) -> Option<ClosestElem> {
        find_closest_rect(&self.sticks, point, transform)
    }

    fn on_hover(
        &self,
        _plot_area_response: &Response,
        elem: ClosestElem,
        shapes: &mut Vec<Shape>,
        cursors: &mut Vec<Cursor>,
        plot: &PlotConfig<'_>,
        _: Option<&LabelFormatterFn<'_>>,
    ) {
        let candlestick_chart: &Candlestick = &self.sticks[elem.index];

        candlestick_chart.add_shapes(plot.transform, true, shapes);
        candlestick_chart.add_rulers_and_text(self, plot, shapes, cursors);
    }

    fn base(&self) -> &PlotItemBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PlotItemBase {
        &mut self.base
    }
}

/// Contains the values of a single [`Candlestick`] in a [`CandlestickChart`].
#[derive(Clone, Debug, PartialEq)]
pub struct Ohlc {
    /// First registered value during that Candle's timeframe.
    pub open: f64,

    /// Highest registered value during that Candle's timeframe.
    pub high: f64,

    /// Lowest registered value during that Candle's timeframe.
    pub low: f64,

    /// Last registered value during that Candle's timeframe.
    pub close: f64,
    //
    // /// Total registered Volume during that Candle's timeframe.
    // pub volume: f64,
}
impl Ohlc {
    pub fn new(
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        // volume: f64
    ) -> Self {
        Self {
            open,
            high,
            low,
            close,
            // volume,
        }
    }
}

/// One floating Candlestick in a [`CandlestickChart`], made of a Body and Whicks.
///
/// Body and Whick width can be changed to allow variable-width histograms.
#[derive(Clone, Debug, PartialEq)]
pub struct Candlestick {
    /// Name of plot element in the diagram (annotated by default formatter).
    pub name: String,

    /// Which direction the candlestick faces in the diagram.
    pub orientation: Orientation,

    /// Position on the argument (input) axis -- X if vertical, Y if horizontal.
    pub argument: f64,

    /// Contains the values of a single [`Candlestick`] in a [`CandlestickChart`].
    pub ohlc: Ohlc,

    /// Thickness of the body
    pub body_width: f64,

    /// Thickness of the whick
    pub whick_width: f64,

    /// Outline width and color of the body
    pub body_stroke: Stroke,

    /// Fill color of the body
    pub body_fill: Color32,

    /// Outline width and color of the whick
    pub whick_stroke: Stroke,

    /// Fill color of the whick
    pub whick_fill: Color32,
}
impl Candlestick {
    /// Create a candlestick. Its `orientation` is set by its [`CandlestickChart`] parent.
    ///
    /// - `argument`: Position on the argument axis (X if vertical, Y if horizontal).
    ///
    /// - `ohlc`: Y Position of the candlestick elements (if vertical).
    pub fn new(argument: f64, ohlc: Ohlc) -> Self {
        Self {
            argument,
            orientation: Orientation::default(),
            name: String::default(),
            ohlc,
            body_width: 0.8,
            whick_width: 0.2,
            body_stroke: Stroke::new(1.0, Color32::TRANSPARENT),
            body_fill: Color32::TRANSPARENT,
            whick_stroke: Stroke::new(1.0, Color32::TRANSPARENT),
            whick_fill: Color32::TRANSPARENT,
        }
    }

    /// Name of this candlestick element.
    #[expect(clippy::needless_pass_by_value, reason = "to allow various string types")]
    #[inline]
    pub fn name(mut self, name: impl ToString) -> Self {
        self.name = name.to_string();
        self
    }

    /// Add a custom stroke to the body.
    #[inline]
    pub fn stroke_body(mut self, stroke: impl Into<Stroke>) -> Self {
        self.body_stroke = stroke.into();
        self
    }

    /// Add a custom stroke to the whicks.
    #[inline]
    pub fn stroke_whick(mut self, stroke: impl Into<Stroke>) -> Self {
        self.whick_stroke = stroke.into();
        self
    }

    /// Add a custom fill color to the body.
    #[inline]
    pub fn fill_body(mut self, color: impl Into<Color32>) -> Self {
        self.body_fill = color.into();
        self
    }

    /// Add a custom fill color to the whicks.
    #[inline]
    pub fn fill_whick(mut self, color: impl Into<Color32>) -> Self {
        self.whick_fill = color.into();
        self
    }

    /// Set the body width.
    #[inline]
    pub fn body_width(mut self, width: f64) -> Self {
        self.body_width = width;
        self
    }

    /// Set the whick width.
    #[inline]
    pub fn whick_width(mut self, width: f64) -> Self {
        self.whick_width = width;
        self
    }

    /// Set orientation of the element as vertical. Argument axis is X.
    #[inline]
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Set orientation of the element as horizontal. Argument axis is Y.
    #[inline]
    pub fn horizontal(mut self) -> Self {
        self.orientation = Orientation::Horizontal;
        self
    }

    pub(in crate::items) fn add_shapes(&self, transform: &PlotTransform, highlighted: bool, shapes: &mut Vec<Shape>) {
        let (new_body_stroke, new_body_fill, new_whick_stroke, new_whick_fill) = if highlighted {
            let (hi_body_stroke, hi_body_fill) = highlighted_color(self.body_stroke, self.body_fill);
            let (hi_whick_stroke, hi_whick_fill) = highlighted_color(self.whick_stroke, self.whick_fill);

            (hi_body_stroke, hi_body_fill, hi_whick_stroke, hi_whick_fill)
        } else {
            (self.body_stroke, self.body_fill, self.whick_stroke, self.whick_fill)
        };

        let (upper, lower) = if self.ohlc.close > self.ohlc.open {
            // usually a green bull candle
            (self.ohlc.close, self.ohlc.open)
        } else {
            // usually a red bear candle
            (self.ohlc.open, self.ohlc.close)
        };

        let half_body_width: f64 = self.body_width / 2.0;

        // Upper Whick
        {
            let rect = transform.rect_from_values(
                &self.point_at(self.argument - half_body_width, upper),
                &self.point_at(self.argument + self.body_width / 2.0, self.ohlc.high), // &self.bounds_max(),
            );
            let high_whick = Shape::Rect(RectShape::new(
                rect,
                CornerRadius::ZERO,
                new_whick_fill,
                new_whick_stroke,
                StrokeKind::Inside,
            ));
            shapes.push(high_whick);
        };

        // Body
        {
            let rect = transform.rect_from_values(
                &self.point_at(self.argument - half_body_width, lower),
                &self.point_at(self.argument + half_body_width, upper),
            );
            let body = Shape::Rect(RectShape::new(
                rect,
                CornerRadius::ZERO,
                new_body_fill,
                new_body_stroke,
                StrokeKind::Inside,
            ));
            shapes.push(body);
        };

        // Lower Whick
        {
            let rect = transform.rect_from_values(
                &self.point_at(self.argument - half_body_width, self.ohlc.low), // &self.bounds_min(),
                &self.point_at(self.argument + half_body_width, lower),
            );
            let low_whick = Shape::Rect(RectShape::new(
                rect,
                CornerRadius::ZERO,
                new_whick_fill,
                new_whick_stroke,
                StrokeKind::Inside,
            ));
            shapes.push(low_whick);
        };
    }

    pub(in crate::items) fn add_rulers_and_text(
        &self,
        parent: &CandlestickChart,
        plot: &PlotConfig<'_>,
        shapes: &mut Vec<Shape>,
        cursors: &mut Vec<Cursor>,
    ) {
        let text: Option<String> = parent.element_formatter.as_ref().map(|fmt| fmt(self, parent));

        add_rulers_and_text(self, plot, text, shapes, cursors);
    }
}
impl RectElement for Candlestick {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn bounds_min(&self) -> PlotPoint {
        self.point_at(self.argument - self.body_width / 2.0, self.ohlc.low)
    }

    fn bounds_max(&self) -> PlotPoint {
        self.point_at(self.argument + self.body_width / 2.0, self.ohlc.high)
    }

    fn values_with_ruler(&self) -> Vec<PlotPoint> {
        let open: PlotPoint = self.point_at(self.argument, self.ohlc.open);
        let high: PlotPoint = self.point_at(self.argument, self.ohlc.high);
        let low: PlotPoint = self.point_at(self.argument, self.ohlc.low);
        let close: PlotPoint = self.point_at(self.argument, self.ohlc.close);
        vec![open, high, low, close]
    }

    fn orientation(&self) -> Orientation {
        self.orientation
    }

    fn corner_value(&self) -> PlotPoint {
        self.point_at(self.argument, self.ohlc.high)
    }

    fn default_values_format(&self, _transform: &PlotTransform) -> String {
        if self.ohlc.close > self.ohlc.open {
            // usually a green bull candle, close above open
            format!(
                "High = {high:.2}\
                \nClose = {close:.2}\
                \nOpen = {open:.2}\
                \nLow = {low:.2}",
                high = self.ohlc.high,
                close = self.ohlc.close,
                open = self.ohlc.open,
                low = self.ohlc.low,
            )
        } else {
            // usually a red bear candle, open above close
            format!(
                "High = {high:.2}\
                \nOpen = {open:.2}\
                \nClose = {close:.2}\
                \nLow = {low:.2}",
                high = self.ohlc.high,
                open = self.ohlc.open,
                close = self.ohlc.close,
                low = self.ohlc.low,
            )
        }
    }
}
