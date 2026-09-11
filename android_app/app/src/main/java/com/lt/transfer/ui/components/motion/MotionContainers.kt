package com.lt.transfer.ui.components.motion

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.EaseInCubic
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.IntOffset
import kotlin.math.roundToInt

// 动画时长常量（根据项目实际调整）
// 页签过渡总时长 = PAGE_FADE_OUT_MS + PAGE_FADE_IN_MS = 300ms（Material 3 fade-through 规格）
private const val PAGE_FADE_OUT_MS = 90
private const val PAGE_FADE_IN_MS = 210
// 位移距离占屏宽比例。纯淡入淡出没有方向感，加一点点位移作方向提示；
// 比例必须小：整屏宽度平移时单帧位移过大，文字会产生拖影（时域混叠）。
private const val PAGE_SLIDE_FRACTION = 0.125f
private const val COLLAPSE_DURATION_MS = 220
private const val TEXT_SWITCH_DURATION_MS = 180

/**
 * 页签切换的过渡容器（Material 3 fade-through + 轻微位移）。
 *
 * 叠影的成因是速度而非重叠：整屏平移时若缓动起始斜率过大（如 EaseOutCubic
 * 起始斜率 3.0），首帧位移可达屏宽 15%，文字便呈现拖影。本实现把主要过渡
 * 交给 alpha：退场页在 [PAGE_FADE_OUT_MS] 内淡出，进场页延迟同样时长后淡入，
 * 位移幅度仅 [PAGE_SLIDE_FRACTION] 屏宽且发生在低 alpha 区间，速度低到
 * 不会触发时域混叠。两段时间首尾相接不重叠，任意时刻只有一页可见。
 *
 * @param targetPage 当前目标页索引；索引变大视为「前进」，新页自右侧带入。
 * @param content    页面内容，入参为当前正在渲染的页索引。
 */
@Composable
fun AnimatedPageHost(
    targetPage: Int,
    modifier: Modifier = Modifier,
    content: @Composable (Int) -> Unit,
) {
    // 动画规格与方向无关，提前 remember 复用，避免每次过渡重建。
    // 进场位移时长与其淡入窗口对齐：页面不可见时不做无谓位移。
    val enterFadeSpec = remember {
        tween<Float>(durationMillis = PAGE_FADE_IN_MS, delayMillis = PAGE_FADE_OUT_MS, easing = LinearEasing)
    }
    val enterSlideSpec = remember {
        tween<IntOffset>(durationMillis = PAGE_FADE_IN_MS, delayMillis = PAGE_FADE_OUT_MS, easing = EaseOutCubic)
    }
    val exitFadeSpec = remember {
        tween<Float>(durationMillis = PAGE_FADE_OUT_MS, easing = LinearEasing)
    }
    val exitSlideSpec = remember {
        tween<IntOffset>(durationMillis = PAGE_FADE_OUT_MS, easing = EaseInCubic)
    }

    AnimatedContent(
        targetState = targetPage,
        transitionSpec = {
            // 判断方向：目标页索引 > 初始页索引则为前进（自右带入）
            val direction = if (targetState > initialState) 1 else -1

            (fadeIn(enterFadeSpec) + slideInHorizontally(enterSlideSpec) { fullWidth ->
                (fullWidth * (PAGE_SLIDE_FRACTION * direction)).roundToInt()
            }) togetherWith (fadeOut(exitFadeSpec) + slideOutHorizontally(exitSlideSpec) { fullWidth ->
                (fullWidth * (-PAGE_SLIDE_FRACTION * direction)).roundToInt()
            })
        },
        label = "PageTransition",
        modifier = modifier
            .fillMaxSize()
            // 不透明兜底底色：淡出与淡入的交接瞬间两页 alpha 都接近 0，
            // 没有这层就会露出窗口底色（闪白）。页面自身不铺背景，
            // 避免滑动/淡变期间每帧多画整屏色块造成过度绘制。
            .background(MaterialTheme.colorScheme.background),
    ) { page ->
        content(page)
    }
}

/**
 * 条件内容的平滑展开 / 折叠容器。
 *
 * @param visible 为 true 时展开显示 [content]，为 false 时折叠并移除占位高度。
 */
@Composable
fun AnimatedCollapse(
    visible: Boolean,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    AnimatedVisibility(
        visible = visible,
        enter = expandVertically(animationSpec = tween(COLLAPSE_DURATION_MS, easing = FastOutSlowInEasing)),
        exit = shrinkVertically(animationSpec = tween(COLLAPSE_DURATION_MS, easing = FastOutSlowInEasing)),
        modifier = modifier,
    ) {
        content()
    }
}

/**
 * 文本切换时的淡入淡出过渡，避免数值/字符串变化时生硬跳变。
 *
 * @param targetText 目标文本；变化时旧文本淡出、新文本淡入。
 */
@Composable
fun AnimatedTextSwitcher(
    targetText: String,
    modifier: Modifier = Modifier,
    style: TextStyle = LocalTextStyle.current,
) {
    AnimatedContent(
        targetState = targetText,
        transitionSpec = {
            fadeIn(tween(TEXT_SWITCH_DURATION_MS)) togetherWith fadeOut(tween(TEXT_SWITCH_DURATION_MS))
        },
        label = "TextSwitcher",
        modifier = modifier,
    ) { text ->
        Text(text = text, style = style)
    }
}
